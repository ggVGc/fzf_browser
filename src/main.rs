use anyhow::{anyhow, ensure, Context, Result};
use clap::Parser;
use serde::{Deserialize, Serialize};
use std::ffi::{OsStr, OsString};
use std::io::Write;
use std::path::PathBuf;
use std::process::{ExitCode, Stdio};
use std::{env, fs};
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
use tokio::process::{ChildStdin, Command};
use tokio::time::sleep;

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    #[clap(default_value = ".")]
    start_path: OsString,
    #[clap(short, long, default_value = "")]
    query: String,
    #[clap(short, long, default_value_t = false)]
    recursive: bool,
    /// Pass-through options for fzf
    #[clap(short, long, default_value = "")]
    fzf_opts: String,
}

#[derive(Serialize)]
#[serde(tag = "tag")]
#[serde(rename_all = "snake_case")]
enum Message {
    ClientInit {
        launch_directory: String,
        start_directory: String,
        start_query: String,
        recursive: bool,
    },
    Result {
        query: String,
        key: String,
        selection: Vec<String>,
        code: i32,
    },
}

#[tokio::main]
async fn main() -> Result<ExitCode> {
    let cli = Cli::parse();

    let start_path = resolve(&cli.start_path)?;

    let mut client = UnixStream::connect("/tmp/fuba.socket")
        .await
        .context("connecting to socket")?;

    append_json(
        &mut client,
        &Message::ClientInit {
            launch_directory: path_str(resolve(env::current_dir()?)?)?,
            start_directory: path_str(&start_path)?,
            start_query: cli.query.to_string(),
            recursive: cli.recursive,
        },
    )
    .await?;

    let (u_read, mut u_write) = client.split();
    let mut u_read = BufReader::new(u_read);

    let mut fzf: Option<Fzf> = None;
    let mut read_content = true;

    let mut u_read_buf = Vec::new();
    loop {
        if let Some(fzf_ref) = fzf.as_mut() {
            if let Some(exit_code) = fzf_ref.process.try_wait()? {
                let code = exit_code
                    .code()
                    .ok_or_else(|| anyhow!("fzf exited without a code"))?;

                append_json(
                    &mut u_write,
                    &consume_output(&mut fzf_ref.stdout, code).await?,
                )
                .await?;

                read_content = true;

                let mut fzf = fzf.take().expect("checked above");
                drop(fzf.stdin.take());
                fzf.process.wait().await?;
            }
        }

        if !read_content {
            sleep(std::time::Duration::from_millis(100)).await;
            continue;
        }

        let cmd = read_line(&mut u_read, &mut u_read_buf).await?;

        match cmd[0] {
            b'z' => {
                let _: ChildStdin = fzf
                    .as_mut()
                    .ok_or_else(|| anyhow!("fzf not open"))?
                    .stdin
                    .take()
                    .ok_or_else(|| anyhow!("fzf stdin already taken"))?;
                read_content = false;
            }
            b'x' => {
                std::io::stdout().write_all(&cmd[1..])?;
                return Ok(ExitCode::SUCCESS);
            }
            b'e' => {
                ensure!(cmd.len() == 1, "e command must be empty");

                let fzf = fzf.as_mut().ok_or_else(|| anyhow!("fzf not open"))?;
                let stdin = fzf
                    .stdin
                    .as_mut()
                    .ok_or_else(|| anyhow!("fzf stdin already taken"))?;

                loop {
                    let entry = read_line(&mut u_read, &mut u_read_buf).await?;
                    if entry.is_empty() {
                        break;
                    }
                    stdin.write_all(entry).await?;
                    stdin.write_all(b"\n").await?;
                }
            }
            b'o' => {
                let user_args = serde_json::from_slice(&cmd[1..])
                    .context("parsing user_args from 'o' command")?;
                fzf = Some(open_fzf(&user_args, &cli).await?);
            }
            b'\x1b' => (),
            other => unimplemented!("unknown command: {other:?}"),
        }
    }
}

fn path_str(s: impl AsRef<OsStr>) -> Result<String> {
    let s = s.as_ref();
    Ok(s.to_str()
        .ok_or_else(|| anyhow!("unrepresentable path: {s:?}"))?
        .to_string())
}

fn resolve(s: impl AsRef<OsStr>) -> Result<PathBuf> {
    Ok(fs::canonicalize(s.as_ref())?)
}

async fn append_json(mut writer: impl AsyncWrite + Unpin, value: &impl Serialize) -> Result<()> {
    let mut vec = serde_json::to_vec(value)?;
    vec.push(b'\n');
    writer.write_all(&vec).await?;
    Ok(())
}

async fn read_line<'b>(
    reader: &mut BufReader<impl AsyncReadExt + Unpin>,
    buf: &'b mut Vec<u8>,
) -> Result<&'b [u8]> {
    buf.clear();
    let read = reader.read_until(b'\n', buf).await?;
    ensure!(read > 0, "EOF");
    Ok(&buf[..read - 1])
}

#[derive(Deserialize)]
struct UserArgs {
    prompt_prefix: String,
    query: String,

    #[serde(default)]
    with_ansi_colors: bool,
    #[serde(default)]
    sort: bool,
    preview_command: Option<String>,
    #[serde(default)]
    key_bindings: Vec<String>,
}

async fn open_fzf(user_args: &UserArgs, cli: &Cli) -> Result<Fzf> {
    let mut fzf_args = vec![
        "--prompt".to_string(),
        format!("{}: ", user_args.prompt_prefix),
    ];

    fzf_args.extend(cli.fzf_opts.split_whitespace().map(str::to_string));

    if user_args.with_ansi_colors {
        fzf_args.push("--ansi".to_string());
    }

    if !user_args.sort {
        fzf_args.push("+s".to_string());
    }

    if let Some(preview_command) = &user_args.preview_command {
        fzf_args.push("--preview".to_string());
        fzf_args.push(preview_command.to_string());
    }

    fzf_args.push("--print-query".to_string());
    fzf_args.push("--query".to_string());
    fzf_args.push(user_args.query.to_string());
    fzf_args.push("--multi".to_string());
    fzf_args.push("--extended".to_string());
    fzf_args.push("--tiebreak=chunk,length,end,index".to_string());
    fzf_args.push("--expect".to_string());
    fzf_args.push(user_args.key_bindings.join(","));

    let mut fzf = Command::new("fzf")
        .args(fzf_args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .context("spawning fzf")?;

    let f_read = fzf.stdout.take().expect("specified above");
    let f_write = fzf.stdin.take().expect("specified above");

    Ok(Fzf {
        process: fzf,
        stdin: Some(f_write),
        stdout: f_read,
    })
}

struct Fzf {
    process: tokio::process::Child,
    stdin: Option<tokio::process::ChildStdin>,
    stdout: tokio::process::ChildStdout,
}

async fn consume_output(mut from: impl AsyncRead + Unpin, code: i32) -> Result<Message> {
    let mut fzf_output = String::new();
    from.read_to_string(&mut fzf_output).await?;

    let mut lines = fzf_output
        .split('\n')
        .map(str::to_string)
        .collect::<Vec<_>>();

    ensure!(lines.len() > 3, "no selection made");

    lines.pop();

    Ok(Message::Result {
        query: lines.remove(0),
        key: lines.remove(1),
        selection: lines,
        code,
    })
}
