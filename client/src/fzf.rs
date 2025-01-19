use crate::Message;
use anyhow::{ensure, Context, Result};
use serde::Deserialize;
use std::process::Stdio;
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::process::Command;

pub struct Fzf {
    pub process: tokio::process::Child,
    pub stdin: Option<tokio::process::ChildStdin>,
    pub stdout: tokio::process::ChildStdout,
}

#[derive(Deserialize)]
pub struct UserArgs {
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

pub async fn open_fzf<I: IntoIterator<Item = String>>(
    user_args: &UserArgs,
    options: I,
) -> Result<Fzf> {
    let mut fzf_args = vec![
        "--prompt".to_string(),
        format!("{}: ", user_args.prompt_prefix),
    ];

    fzf_args.extend(options);

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

pub async fn consume_output(mut from: impl AsyncRead + Unpin, code: i32) -> Result<Message> {
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
