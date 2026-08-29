use crate::draw::PreviewMode;
use crate::line_stop::{LineStopFmtWrite, LineStopIoWrite};
use crate::ui_state::URect;
use ansi_to_tui::IntoText;
use anyhow::{anyhow, Result};
use content_inspector::ContentType;
use image::{DynamicImage, GenericImageView, ImageFormat, ImageReader};
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
            ImageOutcome::Rendered(image_content) => image_content,
            ImageOutcome::Failed(message) => message,
            ImageOutcome::NotAnImage => show_binary(&content, &showing, area, coloured)?,
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

enum ImageOutcome<'a> {
    /// decoded and drawn
    Rendered(Text<'a>),
    /// it is an image, but we couldn't show it; explain why
    Failed(Text<'a>),
    /// not an image at all, fall back to the binary view
    NotAnImage,
}

/// identify a file by its content, not its extension; a webp named .png must
/// not reach the png decoder. `Ok(None)` means it isn't a recognised image.
///
/// note that `ImageReader::open()` would seed the format from the extension,
/// and only replace it if sniffing succeeds, so it isn't usable here
fn sniff_image(path: &Path) -> Result<Option<(ImageReader<io::BufReader<fs::File>>, ImageFormat)>> {
    let reader = ImageReader::new(io::BufReader::new(fs::File::open(path)?)).with_guessed_format()?;

    Ok(reader.format().map(|format| (reader, format)))
}

fn show_image<'a>(
    showing: &impl AsRef<Path>,
    area: URect,
) -> Result<ImageOutcome<'a>, anyhow::Error> {
    use termimage::ops;

    let (reader, format) = match sniff_image(showing.as_ref()) {
        Ok(Some(sniffed)) => sniffed,
        Ok(None) => return Ok(ImageOutcome::NotAnImage),
        Err(e) => return Ok(ImageOutcome::Failed(image_error(showing, &e.to_string()))),
    };

    // decoders are third party and not all of them are panic-free on
    // malformed input, so treat a panic as just another decode failure
    let image: DynamicImage = match crate::alt_screen::catch_quiet_panic(|| reader.decode()) {
        Ok(Ok(image)) => image,
        Ok(Err(e)) => {
            return Ok(ImageOutcome::Failed(image_error(
                showing,
                &format!("{:?}: {}", format, e),
            )))
        }
        Err(payload) => {
            return Ok(ImageOutcome::Failed(image_error(
                showing,
                &format!(
                    "{:?} decoder panicked: {}",
                    format,
                    crate::alt_screen::panic_message(&payload)
                ),
            )))
        }
    };

    let render = crate::alt_screen::catch_quiet_panic(|| -> Result<Text<'static>> {
        let size = (area.width as u32, area.height as u32);
        let img_s = ops::image_resized_size(image.dimensions(), size, true);
        let resized = ops::resize_image(&image, img_s);

        // not a LineStopIoWrite: termimage unwraps writer errors, and the
        // resize above already bounds this to the preview area
        let mut writer = Vec::with_capacity(32 * area.height);
        ops::write_ansi_truecolor(&mut writer, &resized);

        let mut text = writer.into_text()?;
        text.lines.truncate(area.height);
        Ok(text)
    });

    Ok(match render {
        Ok(Ok(text)) => ImageOutcome::Rendered(text),
        Ok(Err(e)) => ImageOutcome::Failed(image_error(showing, &e.to_string())),
        Err(payload) => ImageOutcome::Failed(image_error(
            showing,
            &format!(
                "rendering panicked: {}",
                crate::alt_screen::panic_message(&payload)
            ),
        )),
    })
}

fn image_error<'a>(showing: &impl AsRef<Path>, why: &str) -> Text<'a> {
    Text::from(vec![
        preview_header("image", showing),
        Line::default(),
        Line::from(Span::styled(
            "can't display this image",
            Style::new().light_red().bold(),
        )),
        Line::from(Span::styled(why.to_string(), Style::new().dim())),
    ])
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

#[cfg(test)]
mod tests {
    use super::*;

    const AREA: URect = URect {
        x: 0,
        y: 0,
        width: 80,
        height: 24,
    };

    fn write_temp(name: &str, content: &[u8]) -> PathBuf {
        let path = std::env::temp_dir().join(format!("rurt-test-{name}"));
        fs::write(&path, content).expect("writing fixture");
        path
    }

    fn webp_bytes() -> Vec<u8> {
        let image = DynamicImage::new_rgba8(8, 6);
        let mut out = io::Cursor::new(Vec::new());
        image
            .write_to(&mut out, image::ImageFormat::WebP)
            .expect("encoding");
        out.into_inner()
    }

    /// the extension says png, the bytes say webp; the bytes win
    #[test]
    fn mislabelled_image_renders() {
        let path = write_temp("mislabelled.png", &webp_bytes());
        match show_image(&path, AREA).expect("no hard error") {
            ImageOutcome::Rendered(text) => assert!(!text.lines.is_empty()),
            _ => panic!("expected the webp to render"),
        }
    }

    /// an image we can't decode explains itself instead of panicking
    #[test]
    fn corrupt_image_explains() {
        let mut content = webp_bytes();
        content.truncate(content.len() / 2);
        let path = write_temp("corrupt.webp", &content);
        match show_image(&path, AREA).expect("no hard error") {
            ImageOutcome::Failed(text) => {
                let rendered = text
                    .lines
                    .iter()
                    .map(|l| l.to_string())
                    .collect::<Vec<_>>()
                    .join("\n");
                assert!(rendered.contains("can't display this image"), "{rendered}");
            }
            _ => panic!("expected a failure message"),
        }
    }

    /// a non-image still falls through to the hex view
    #[test]
    fn other_binary_falls_through() {
        let path = write_temp("binary.png", &[0u8, 1, 2, 3, 255, 254, 253]);
        assert!(matches!(
            show_image(&path, AREA).expect("no hard error"),
            ImageOutcome::NotAnImage
        ));
    }
}
