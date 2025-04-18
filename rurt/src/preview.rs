use crate::draw::PreviewMode;
use crate::line_stop::{LineStopFmtWrite, LineStopIoWrite};
use crate::ui_state::URect;
use ansi_to_tui::IntoText;
use anyhow::{anyhow, Result};
use content_inspector::ContentType;
use image::{DynamicImage, GenericImageView};
use ratatui::prelude::*;
use std::collections::VecDeque;
use std::ffi::OsStr;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};
use std::{fs, io};

#[derive(Default)]
pub struct Previews {
    pub inner: VecDeque<Preview>,
}

pub struct Preview {
    pub showing: PathBuf,
    pub mode: PreviewMode,
    pub target_area: URect,
    pub coloured: bool,
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
    coloured: bool,
    mode: PreviewMode,
    preview: Arc<Mutex<PreviewedData>>,
    area: URect,
) -> Result<()> {
    match mode {
        PreviewMode::Content => run_preview_content(pathref, coloured, preview, area),
        PreviewMode::GitLg => run_git(pathref, coloured, preview, area, "lg"),
        PreviewMode::GitShow => run_git(pathref, coloured, preview, area, "show"),
    }
}

fn run_preview_content(
    pathref: impl AsRef<Path>,
    coloured: bool,
    preview: Arc<Mutex<PreviewedData>>,
    area: URect,
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

        let rendered = interpret_file(content, path, area, coloured)?;
        preview.lock().expect("panic").render = Some(rendered);

        return Ok(());
    }

    let command = "ls";
    preview.lock().expect("panic").command = PreviewCommand::Custom(command.to_string());

    let spawn = Command::new(command)
        .args([
            path.as_os_str(),
            OsStr::new("-al"),
            if coloured {
                OsStr::new("--color=always")
            } else {
                OsStr::new("--color=never")
            },
        ])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()?;

    let mut buf = Vec::with_capacity(4096);
    spawn
        .stdout
        .expect("piped")
        .take(1024 * 1024)
        .read_to_end(&mut buf)?;

    let mut text = indent(&buf, b"     ")?;
    text.lines.insert(0, preview_header("ls", path));

    let mut preview = preview.lock().expect("panic");
    preview.render = Some(text);
    preview.content = buf;
    Ok(())
}

fn indent(buf: &[u8], with: &[u8]) -> Result<Text<'static>> {
    let mut indented = Vec::with_capacity(buf.len() * 2);
    for line in buf.split(|&b| b == b'\n') {
        indented.extend_from_slice(with);
        indented.extend_from_slice(line);
        indented.push(b'\n');
    }
    indented.trim_ascii_end();
    Ok(indented.into_text()?)
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
    area: URect,
    coloured: bool,
) -> Result<Text<'static>> {
    use ansi_to_tui::IntoText as _;

    Ok(match content_inspector::inspect(&content) {
        ContentType::BINARY => match show_image(&showing, area)? {
            Some(image_content) => image_content,
            None => show_binary(&content, &showing, area, coloured)?,
        },
        _ => {
            let mut writer = LineStopFmtWrite::new(area.height);
            content.retain(|&b| b != b'\r');
            // expecting an unnamed error on writer full
            let _ = bat::PrettyPrinter::new()
                .input(bat::Input::from_bytes(&content).name(&showing))
                .header(false)
                .colored_output(coloured)
                .term_width(area.width)
                .tab_width(Some(2))
                .line_numbers(true)
                .use_italics(false)
                .print_with_writer(Some(&mut writer));
            let mut ret = writer.inner.into_text()?;
            ret.lines.insert(0, preview_header("bat", showing));
            ret
        }
    })
}

fn show_image<'a>(
    showing: &impl AsRef<Path>,
    area: URect,
) -> Result<Option<Text<'a>>, anyhow::Error> {
    use termimage::ops;

    let image: Option<DynamicImage> = {
        let description = (String::new(), showing.as_ref().to_path_buf());
        if let Some(format) = ops::guess_format(&description).ok() {
            ops::load_image(&description, format).ok()
        } else {
            None
        }
    };

    if let Some(image) = image {
        let size = (area.width as u32, area.height as u32);
        let img_s = ops::image_resized_size(image.dimensions(), size, true);
        let resized = ops::resize_image(&image, img_s);

        let mut writer = LineStopIoWrite::new(area.height);
        ops::write_ansi_truecolor(&mut writer, &resized);

        Ok(Some(writer.inner.into_text()?))
    } else {
        Ok(None)
    }
}

fn show_binary<'a>(
    content: &Vec<u8>,
    showing: &impl AsRef<Path>,
    area: URect,
    coloured: bool,
) -> Result<Text<'a>, anyhow::Error> {
    let mut v = LineStopIoWrite::new(area.height);
    let panels = (area.width.saturating_sub(10) / 35).max(1);
    let _ = hexyl::PrinterBuilder::new(&mut v)
        .num_panels(panels as u64)
        .show_color(coloured)
        .build()
        .print_all(io::Cursor::new(content));
    let mut ret = v.inner.into_text()?;
    ret.lines.insert(0, preview_header("hexyl", showing));
    let media_type = file_type::FileType::from_bytes(content);
    if !media_type.extensions().is_empty() {
        ret.lines.insert(0, preview_header("file", showing));
        ret.lines.insert(
            1,
            Line::from(Span::styled(media_type.name(), Style::new().dim())),
        );
        ret.lines.insert(2, Line::default());
    }
    Ok(ret)
}

pub fn preview_header(command: &str, showing: impl AsRef<Path>) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("{:>5}", command), Style::new().light_yellow()),
        Span::raw(" "),
        Span::styled(showing.as_ref().display().to_string(), Style::new().bold()),
    ])
}

impl Previews {
    pub fn is_scanning(&self) -> bool {
        self.inner.iter().any(|v| !v.worker.is_finished())
    }

    pub fn would_flicker(&self) -> bool {
        self.inner
            .iter()
            .any(|v| v.started.elapsed() < Duration::from_millis(100) && !v.worker.is_finished())
    }
}

fn run_git(
    path: impl AsRef<Path>,
    coloured: bool,
    preview: Arc<Mutex<PreviewedData>>,
    _area: URect,
    sub_cmd: &str,
) -> Result<()> {
    preview.lock().expect("panic").command = PreviewCommand::Custom(format!("g {sub_cmd}"));

    let spawn = Command::new("git")
        .args([
            OsStr::new(sub_cmd),
            if coloured {
                OsStr::new("--color=always")
            } else {
                OsStr::new("--color=never")
            },
            path.as_ref().as_os_str(),
        ])
        .current_dir(path.as_ref().parent().ok_or_else(|| anyhow!("no parent"))?)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()?;

    let mut buf = Vec::with_capacity(4096);
    spawn
        .stdout
        .expect("piped")
        .take(1024 * 1024)
        .read_to_end(&mut buf)?;

    spawn
        .stderr
        .expect("piped")
        .take(1024 * 1024)
        .read_to_end(&mut buf)?;

    buf.retain(|&b| b != b'\r');

    let mut text = indent(&buf, b" ")?;
    text.lines
        .insert(0, preview_header(&format!("g {sub_cmd}"), path));

    preview.lock().expect("panic").render = Some(text);

    Ok(())
}
