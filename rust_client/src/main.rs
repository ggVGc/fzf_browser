use anyhow::{anyhow, bail, ensure, Context, Result};
use clap::Parser;
use serde::{Deserialize, Serialize};
use std::ffi::{OsStr, OsString};
use std::io::Write;
use std::path::PathBuf;
use std::process::{ExitCode, ExitStatus, Stdio};
use std::{env, fs};
use tempfile::TempDir;
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
use tokio::process::Command;
use tokio::select;

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    #[clap(default_value = ".")]
    start_path: OsString,
    #[clap(short, long, default_value = "")]
    query: String,
    #[clap(short, long, default_value_t = false)]
    recursive: bool,
    #[clap(short, long, default_value = "")]
    mode: String,
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
        stream_socket: String,
        recursive: bool,
        file_mode: String,
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

    let mut client = UnixStream::connect(
        dirs::home_dir()
            .ok_or_else(|| anyhow!("no home directory"))?
            .join(".fuba.socket"),
    )
    .await
    .context("connecting to socket")?;

    let temp_dir = TempDir::new()?;
    let socket_path = temp_dir.path().join("stream.socket");
    let stream_server = tokio::net::UnixListener::bind(&socket_path)?;

    append_json(
        &mut client,
        &Message::ClientInit {
            launch_directory: path_str(resolve(env::current_dir()?)?)?,
            start_directory: path_str(&start_path)?,
            start_query: cli.query.to_string(),
            stream_socket: path_str(&socket_path)?,
            recursive: cli.recursive,
            file_mode: cli.mode.to_string(),
        },
    )
    .await?;

    let (u_read, mut u_write) = client.split();
    let mut u_read = BufReader::new(u_read);

    let mut fzf: Option<Fzf> = None;

    let mut u_read_buf = Vec::new();

    loop {
        u_read_buf.clear();

        loop {
            select! {
                read = u_read.read_until(b'\n', &mut u_read_buf) => {
                    if read? == 0 {
                        return Ok(ExitCode::SUCCESS);
                    }
                    break;
                }
                exit_status = wait_if_set(&mut fzf) => {
                    handle_shutdown(
                        &mut u_write,
                        exit_status?,
                        fzf.take().expect("was present during wait"),
                    ).await?;
                }
            }
        }

        let _trailing_newline = u_read_buf.pop();
        let cmd = &u_read_buf;
        match cmd[0] {
            b'x' => {
                std::io::stdout().write_all(&cmd[1..])?;
                return Ok(ExitCode::SUCCESS);
            }
            b'o' => {
                let user_args = serde_json::from_slice(&cmd[1..])
                    .context("parsing user_args from 'o' command")?;
                let (new_fzf, mut fzf_stdin) = open_fzf(&user_args, &cli).await?;
                fzf = Some(new_fzf);
                let (stream, _) = stream_server.accept().await?;

                tokio::spawn(async move {
                    let (mut stream, mut stream_write) = stream.into_split();
                    stream_write.shutdown().await?;
                    tokio::io::copy(&mut stream, &mut fzf_stdin).await?;

                    Ok::<_, anyhow::Error>(())
                });
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

async fn open_fzf(user_args: &UserArgs, cli: &Cli) -> Result<(Fzf, tokio::process::ChildStdin)> {
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

    Ok((
        Fzf {
            process: fzf,
            stdout: f_read,
        },
        f_write,
    ))
}

struct Fzf {
    process: tokio::process::Child,
    stdout: tokio::process::ChildStdout,
}

async fn handle_shutdown(
    u_write: impl AsyncWrite + Unpin,
    exit_status: ExitStatus,
    mut fzf: Fzf,
) -> Result<()> {
    let code = match exit_status.code() {
        Some(code) => code,
        None => bail!("fzf exited with signal"),
    };

    let output = consume_output(&mut fzf.stdout, code).await?;

    append_json(u_write, &output).await?;

    Ok(())
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
        key: lines.remove(0),
        selection: lines,
        code,
    })
}

async fn wait_if_set(fzf: &mut Option<Fzf>) -> Result<ExitStatus> {
    if let Some(fzf) = fzf {
        fzf.process.wait().await.context("waiting for fzf")
    } else {
        std::future::pending().await
    }
}
