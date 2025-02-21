use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use anyhow::Result;

pub struct Preview {
    pub showing: PathBuf,
    pub content: Arc<Mutex<Vec<u8>>>,
    pub worker: Option<JoinHandle<()>>,
}

pub fn run_preview(path: impl AsRef<Path>, content: Arc<Mutex<Vec<u8>>>) -> Result<()> {
    let path = path.as_ref().to_path_buf();
    let spawn = Command::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../fzf-browser-preview.sh"
    ))
    .args(&[path])
    .stdout(std::process::Stdio::piped())
    .stderr(std::process::Stdio::null())
    .spawn()?;

    let mut stdout = spawn.stdout.expect("piped");
    let mut buf = [0u8; 1024];
    loop {
        let bytes = stdout.read(&mut buf)?;
        if bytes == 0 {
            break;
        }
        let buf = &buf[..bytes];
        let mut content = content.lock().expect("panic");
        content.extend(buf);
        if content.len() > 1024 * 1024 {
            break;
        }
    }
    Ok(())
}
