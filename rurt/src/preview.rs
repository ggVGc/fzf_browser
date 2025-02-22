use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use anyhow::{anyhow, Result};

pub struct Preview {
    pub showing: PathBuf,
    pub content: Arc<Mutex<PreviewedData>>,
    pub worker: Option<JoinHandle<()>>,
}

#[derive(Default)]
pub struct PreviewedData {
    pub command: String,
    pub content: Vec<u8>,
}

pub fn run_preview(pathref: impl AsRef<Path>, preview: Arc<Mutex<PreviewedData>>) -> Result<()> {
    let path = pathref.as_ref();
    let command = if path.is_file() { "cat" } else { "ls" };
    let spawn = Command::new(command)
        .args(&[path])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()?;

    {
        let mut preview = preview.lock().expect("panic");
        preview.command = command.to_owned();
    }
    let mut stdout = spawn.stdout.expect("piped");
    let mut buf = [0u8; 1024];
    loop {
        let bytes = stdout.read(&mut buf)?;
        if bytes == 0 {
            break;
        }
        let buf = &buf[..bytes];
        let mut preview = preview.lock().expect("panic");
        preview.content.extend(buf);
        if preview.content.len() > 1024 * 1024 {
            break;
        }
    }
    Ok(())
}
