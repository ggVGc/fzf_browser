mod fzf;

use crate::fzf::*;
use anyhow::{anyhow, bail, ensure, Context, Result};
use clap::Parser;
// use log::info;
use serde::Serialize;
use std::ffi::{OsStr, OsString};
use std::io::Write;
use std::path::PathBuf;
use std::process::{ExitCode, ExitStatus};
use std::{env, fs};
use tokio::io::{AsyncBufReadExt, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
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
            file_mode: cli.mode.to_string(),
        },
    )
    .await?;

    let (u_read, mut u_write) = client.split();
    let mut u_read = BufReader::new(u_read);

    enum Mode {
        Command,
        Streaming,
    }

    let mut mode = Mode::Command;

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

        match mode {
            Mode::Streaming => {
                if u_read_buf.len() <= 1 {
                    mode = Mode::Command;
                    continue;
                }

                fzf.as_mut()
                    .ok_or_else(|| anyhow!("fzf not open"))?
                    .stdin
                    .as_mut()
                    .ok_or_else(|| anyhow!("fzf stdin already taken"))?
                    .write_all(&u_read_buf)
                    .await?;

                continue;
            }

            Mode::Command => (),
        }

        let _trailing_newline = u_read_buf.pop();
        let cmd = &u_read_buf;
        match cmd[0] {
            b'z' => {
                drop(
                    fzf.as_mut()
                        .ok_or_else(|| anyhow!("fzf not open"))?
                        .stdin
                        .take(),
                );
            }
            b'x' => {
                std::io::stdout().write_all(&cmd[1..])?;
                return Ok(ExitCode::SUCCESS);
            }
            b'e' => {
                ensure!(
                    cmd.len() == 1,
                    "e command must be empty, got {:?}",
                    String::from_utf8_lossy(cmd)
                );

                mode = Mode::Streaming;
            }
            b'o' => {
                let user_args = serde_json::from_slice(&cmd[1..])
                    .context("parsing user_args from 'o' command")?;
                let fzf_options = cli.fzf_opts.split_whitespace().map(str::to_string);
                fzf = Some(open_fzf(&user_args, fzf_options).await?);
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

async fn wait_if_set(fzf: &mut Option<Fzf>) -> Result<ExitStatus> {
    if let Some(fzf) = fzf {
        fzf.process.wait().await.context("waiting for fzf")
    } else {
        std::future::pending().await
    }
}
