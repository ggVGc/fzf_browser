use crate::preview::{run_preview, Preview, PreviewedData, Previews};
use log::info;
use lscolors::LsColors;
use ratatui::layout::Rect;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Instant;
use tui_input::Input;

pub struct Ui {
    pub input: Input,
    pub view_start: u32,
    pub cursor: u32,
    pub cursor_showing: Option<PathBuf>,
    pub prompt: String,
    pub active: bool,
    pub sorted_items: Vec<u32>,
    pub sorted_until: usize,
    pub previews: Previews,
    pub preview_colours: bool,
    pub ls_colors: LsColors,
}

pub fn fire_preview(ui: &mut Ui, preview_area: Rect) {
    if preview_area.width == 0 || preview_area.height == 0 {
        return;
    }

    let showing = match ui.cursor_showing {
        Some(ref v) => v,
        None => return,
    };

    let started = Instant::now();

    if ui.previews.inner.iter().rev().any(|v| {
        Some(&v.showing) == ui.cursor_showing.as_ref()
            && v.target_area == preview_area
            && v.coloured == ui.preview_colours
    }) {
        return;
    }

    if ui.previews.inner.len() >= 16 {
        ui.previews.inner.pop_front();
    }

    let data = Arc::new(Mutex::new(PreviewedData::default()));

    let write_to = Arc::clone(&data);
    let preview_path = showing.to_path_buf();
    let coloured = ui.preview_colours;
    let area = preview_area;
    let worker = thread::spawn(move || {
        if let Err(e) = run_preview(&preview_path, coloured, Arc::clone(&write_to), area) {
            write_to
                .lock()
                .expect("panic")
                .content
                .extend_from_slice(format!("Error: {}\n", e).as_bytes());
        }
        info!("preview: {preview_path:?} took {:?}", started.elapsed());
    });

    ui.previews.inner.push_back(Preview {
        showing: showing.to_path_buf(),
        target_area: preview_area,
        coloured: ui.preview_colours,
        data,
        worker,
        started,
    });
}
