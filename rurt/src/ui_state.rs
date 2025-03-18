use crate::draw::PreviewMode;
use crate::git::Git;
use crate::item::Item;
use crate::preview::{run_preview, Preview, PreviewedData, Previews};
use log::info;
use lscolors::LsColors;
use ratatui::layout::Rect;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Instant;
use tui_input::Input;

pub struct Ui {
    pub boot: Instant,
    pub input: Input,
    pub view_start: u32,
    pub cursor: Cursor,
    pub cursor_showing: Option<Item>,
    pub prompt: String,
    pub active: bool,
    pub sorted_items: SortedItems,
    pub previews: Previews,
    pub git_info: Option<Git>,
    pub preview_cursor: usize,
    pub preview_colours: bool,
    pub ls_colors: LsColors,
}

impl Ui {
    pub fn is_searching(&self) -> bool {
        !self.input.value().is_empty()
    }

    pub fn cursor_showing_path(&self) -> Option<&Path> {
        self.cursor_showing.as_ref().and_then(|v| v.path())
    }
}

#[derive(Default)]
pub struct Cursor {
    pub last_pos: u32,
    pub pending_move: Option<isize>,
}

pub fn matching_preview(ui: &Ui, mode: PreviewMode) -> Option<&Preview> {
    ui.previews.inner.iter().rev().find(|v| {
        Some(v.showing.as_path()) == ui.cursor_showing_path()
            && v.mode == mode
            && v.coloured == ui.preview_colours
    })
}

pub fn fire_preview(ui: &mut Ui, mode: PreviewMode, preview_area: Rect) {
    if preview_area.width == 0 || preview_area.height == 0 {
        return;
    }

    let mut area = URect::from(preview_area);

    // to facilitate scrolling
    let breakpoint = 400;
    let proposal = area.height * 2 + ui.preview_cursor;

    area.height = (proposal / breakpoint + 1) * breakpoint;

    // BORROW CHECKER
    let showing = match ui.cursor_showing_path().map(|v| v.to_path_buf()) {
        Some(v) => v,
        None => return,
    };

    let started = Instant::now();

    if ui.previews.inner.iter().rev().any(|v| {
        Some(v.showing.as_path()) == ui.cursor_showing_path()
            && v.target_area == area
            && v.mode == mode
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
    let worker = thread::spawn(move || {
        if let Err(e) = run_preview(&preview_path, coloured, mode, Arc::clone(&write_to), area) {
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
        mode,
        target_area: area,
        coloured: ui.preview_colours,
        data,
        worker,
        started,
    });
}

#[derive(Copy, Clone, Eq, PartialEq)]
pub struct URect {
    pub x: usize,
    pub y: usize,
    pub width: usize,
    pub height: usize,
}

impl From<Rect> for URect {
    fn from(r: Rect) -> Self {
        Self {
            x: usize::from(r.x),
            y: usize::from(r.y),
            width: usize::from(r.width),
            height: usize::from(r.height),
        }
    }
}

#[derive(Default)]
pub struct SortedItems {
    pub items: Vec<u32>,
    pub until: u32,
}

impl SortedItems {
    pub fn clear(&mut self) {
        self.items.clear();
        self.until = 0;
    }
}
