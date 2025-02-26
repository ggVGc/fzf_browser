use crate::line_stop::{LineStopFmtWrite, LineStopIoWrite};
use anyhow::Result;
use content_inspector::ContentType;
use ratatui::prelude::*;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Instant;
use std::{fs, io};

pub struct Preview {
    pub showing: PathBuf,
    pub target_area: Rect,
    pub data: Arc<Mutex<PreviewedData>>,
    pub worker: JoinHandle<()>,
    pub started: Instant,
}

#[derive(Default)]
pub enum PreviewCommand {
    #[default]
    Thinking,
    Custom(String),
    InterpretFile,
}

#[derive(Default)]
pub struct PreviewedData {
    pub command: PreviewCommand,
    pub content: Vec<u8>,
    pub render: Option<Text<'static>>,
}

pub fn run_preview(
    pathref: impl AsRef<Path>,
    preview: Arc<Mutex<PreviewedData>>,
    area: Rect,
) -> Result<()> {
    let path = pathref.as_ref();
    if path.is_file() {
        {
            let mut preview = preview.lock().expect("panic");
            preview.command = PreviewCommand::InterpretFile;
        }
        stream_some(fs::File::open(path)?, Arc::clone(&preview))?;

        let read_content = preview.lock().expect("panic");
        let content = read_content.content.clone();
        drop(read_content);

        let rendered = interpret_file(content, path, area)?;
        preview.lock().expect("panic").render = Some(rendered);

        return Ok(());
    }

    let command = "ls";
    let spawn = Command::new(command)
        .args(&[path])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()?;

    {
        let mut preview = preview.lock().expect("panic");
        preview.command = PreviewCommand::Custom(command.to_string());
    }
    stream_some(spawn.stdout.expect("piped"), preview)?;
    Ok(())
}

fn stream_some(reader: impl Read, preview: Arc<Mutex<PreviewedData>>) -> Result<()> {
    let mut reader = reader;
    let mut buf = [0u8; 1024];
    loop {
        let bytes = reader.read(&mut buf)?;
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

fn interpret_file(
    mut content: Vec<u8>,
    showing: impl AsRef<Path>,
    area: Rect,
) -> Result<Text<'static>> {
    use ansi_to_tui::IntoText as _;

    Ok(match content_inspector::inspect(&content) {
        ContentType::BINARY => {
            let mut v = LineStopIoWrite::new(area.height as usize);
            let panels = (area.width.saturating_sub(10) / 35).max(1);
            // TODO: expecting suspicious broken pipe on writer full
            let _ = hexyl::PrinterBuilder::new(&mut v)
                .num_panels(panels as u64)
                .build()
                .print_all(io::Cursor::new(&content));
            v.inner.into_text()?
        }
        _ => {
            let mut writer = LineStopFmtWrite::new(area.height as usize);
            content.retain(|&b| b != b'\r');
            // expecting an unnamed error on writer full
            let _ = bat::PrettyPrinter::new()
                .input(bat::Input::from_bytes(&content).name(showing))
                .header(true)
                .term_width(area.width as usize)
                .tab_width(Some(2))
                .line_numbers(true)
                .use_italics(false)
                .print_with_writer(Some(&mut writer));
            writer.inner.into_text()?
        }
    })
}
