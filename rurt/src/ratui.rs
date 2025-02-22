use crate::action::{handle_action, matches_binding, ActionResult};
use crate::item::Item;
use crate::preview::{run_preview, Preview};
use crate::store::Store;
use crate::App;
use anyhow::Result;
use crossterm::event;
use crossterm::event::Event;
use nucleo::pattern::{CaseMatching, Normalization};
use nucleo::Snapshot;
use ratatui::prelude::*;
use ratatui::widgets::Paragraph;
use std::ops::Deref;
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tui_input::backend::crossterm::to_input_request;
use tui_input::Input;

pub struct Ui {
    pub input: Input,
    pub cursor: u32,
    pub prompt: String,
    pub active: bool,
    pub sorted_items: Vec<u32>,
    pub preview: Preview,
}

pub fn run(store: &mut Store, app: &mut App) -> Result<(Option<String>, ExitCode)> {
    let mut terminal = ratatui::try_init()?;
    let _restore = DropRestore {};

    let mut ui = Ui {
        input: Input::default(),
        cursor: 0,
        prompt: format!("{}> ", app.here.display()),
        active: true,
        sorted_items: Vec::new(),
        preview: Preview {
            showing: PathBuf::new(),
            content: Arc::new(Mutex::new(Vec::new())),
            worker: None,
        },
    };

    store.start_scan(&app)?;

    loop {
        maybe_update_target_dir(app);

        store.nucleo.tick(10);

        let snap = store.nucleo.snapshot();

        ui.active = store.is_scanning();

        terminal.draw(|f| draw_ui(f, &mut ui, snap))?;

        if !event::poll(Duration::from_millis(if ui.active { 5 } else { 90 }))? {
            continue;
        }

        let ev = event::read()?;

        let binding_action = match ev {
            Event::Key(key) => matches_binding(&app.bindings, key),
            _ => None,
        };

        match binding_action {
            Some(action) => {
                let (action, item) = handle_action(action, app, &mut ui, snap)?;
                match action {
                    ActionResult::Ignored => (),
                    ActionResult::Configured => {
                        if let Some(item) = item {
                            if let Some(path) = item.path() {
                                if ui.preview.showing != path {
                                    ui.preview.showing = path.to_owned();
                                    ui.preview.content = Arc::new(Mutex::new(Vec::new()));
                                    let write_to = Arc::clone(&ui.preview.content);
                                    let path = path.to_owned();
                                    std::thread::spawn(move || {
                                        if let Err(e) = run_preview(&path, Arc::clone(&write_to)) {
                                            write_to.lock().expect("panic").extend_from_slice(
                                                format!("Error: {}\n", e).as_bytes(),
                                            );
                                        }
                                    });
                                }
                            }
                        }
                    }
                    ActionResult::Navigated => {
                        app.read_opts.expansions.clear();
                        ui.prompt = format!("{}> ", app.here.display());
                        ui.sorted_items.clear();
                        store.start_scan(app)?;
                    }
                    ActionResult::JustRescan => {
                        ui.sorted_items.clear();
                        store.start_scan(app)?;
                    }
                    ActionResult::Exit(msg, code) => return Ok((msg, code)),
                }
            }
            None => {
                if let Some(req) = to_input_request(&ev) {
                    if ui.input.handle(req).map(|v| v.value).unwrap_or_default() {
                        store.nucleo.pattern.reparse(
                            0,
                            ui.input.value(),
                            CaseMatching::Smart,
                            Normalization::Smart,
                            false,
                        );
                    }
                }
            }
        }
    }
}

fn draw_ui(f: &mut Frame, ui: &mut Ui, snap: &Snapshot<Item>) {
    let [input_line_area, main_app_area] = Layout::default()
        .direction(Direction::Vertical)
        .constraints(&[Constraint::Length(1), Constraint::Min(0)])
        .split(f.area())
        .deref()
        .try_into()
        .expect("static constraints");

    let [left_pane_area, divider_area, right_pane_area] = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(&[
            Constraint::Percentage(50),
            Constraint::Length(1),
            Constraint::Percentage(50),
        ])
        .split(main_app_area)
        .deref()
        .try_into()
        .expect("static constraints");

    draw_input_line(f, &ui.prompt, &mut ui.input, input_line_area);

    draw_listing(f, ui, snap, left_pane_area);
    draw_divider(f, divider_area);
    draw_preview(f, ui, right_pane_area);
}

fn draw_listing(f: &mut Frame, ui: &mut Ui, snap: &Snapshot<Item>, area: Rect) {
    // TODO: not correct; allows positioning one past the end
    ui.cursor = ui.cursor.min(snap.matched_item_count());

    let mut lines = Vec::new();
    lines.push(Line::styled(
        format!(
            "{} {}/{}",
            if ui.active { "S" } else { " " },
            snap.matched_item_count(),
            snap.item_count()
        ),
        Style::new().light_yellow(),
    ));
    let to_show = u32::from(area.height).min(snap.matched_item_count());
    let items = item_range(snap, 0, to_show, ui);

    for (i, item) in items.into_iter().enumerate() {
        let mut spans = Vec::new();
        let selected = ui.cursor as usize == i;
        if selected {
            spans.push(Span::styled("> ", Style::new().light_red().on_dark_gray()));
        } else {
            spans.push(Span::styled(" ", Style::new().on_dark_gray()));
            spans.push(Span::from(" "));
        }

        spans.push(item.as_span(selected));
        lines.push(Line::from(spans));
    }
    f.render_widget(Text::from(lines), area);
}

#[inline]
pub fn item_range<'s>(
    snap: &'s Snapshot<Item>,
    start: u32,
    mut end: u32,
    ui: &mut Ui,
) -> Vec<&'s Item> {
    if end > snap.matched_item_count() {
        end = snap.matched_item_count();
    }
    if start >= end {
        return Vec::new();
    }
    let sort = ui.input.value().is_empty();
    if !sort {
        snap.matched_items(start..end)
            .map(|item| item.data)
            .collect()
    } else {
        let real_end = snap.item_count();
        let cache_end = ui.sorted_items.len() as u32;
        let could_extend = real_end > cache_end;
        let should_extend = end * 2 > cache_end || real_end % 64 == 0;
        if could_extend && should_extend {
            ui.sorted_items.extend(cache_end..real_end);

            if end < real_end {
                ui.sorted_items
                    .select_nth_unstable_by_key(end as usize, |&i| {
                        snap.get_item(i).expect("<end").data
                    });
            }

            ui.sorted_items[0..end as usize]
                .sort_unstable_by_key(|&i| snap.get_item(i).expect("<end").data);
        }

        ui.sorted_items[start as usize..end as usize]
            .into_iter()
            .map(|&i| snap.get_item(i).expect("<end").data)
            .collect()
    }
}

fn draw_input_line(f: &mut Frame, prompt: &str, input: &mut Input, input_line_area: Rect) {
    let mut prompt = Span::styled(prompt, Style::new().light_blue());
    let mut input_line_remainder = input_line_area.width.saturating_sub(prompt.width() as u16);
    if input_line_remainder < 10 {
        prompt = Span::styled("> ", Style::new().blue());
        input_line_remainder = input_line_area.width.saturating_sub(2);
    }

    let prompt_used = prompt.width() as u16;

    f.render_widget(Paragraph::new(prompt), input_line_area);

    let text_area = Rect::new(
        prompt_used,
        input_line_area.y,
        input_line_remainder,
        input_line_area.height,
    );

    let scroll = input.visual_scroll(usize::from(input_line_area.width));
    f.render_widget(
        Paragraph::new(input.value()).scroll((0, scroll as u16)),
        text_area,
    );
    f.set_cursor_position((
        text_area.x + (input.visual_cursor().max(scroll) - scroll) as u16,
        text_area.y,
    ));
}

fn draw_divider(f: &mut Frame, divider_area: Rect) {
    assert_eq!(divider_area.width, 1);
    for y in divider_area.y..divider_area.bottom() {
        f.render_widget(
            Span::from("\u{2502}"), // │ (long |)
            Rect::new(divider_area.x, y, 1, 1),
        );
    }
}

fn draw_preview(f: &mut Frame, ui: &mut Ui, right_pane_area: Rect) {
    f.render_widget(
        Paragraph::new(String::from_utf8_lossy(
            &ui.preview.content.lock().expect("panic"),
        )),
        right_pane_area,
    );
}

struct DropRestore {}
impl Drop for DropRestore {
    fn drop(&mut self) {
        ratatui::restore();
    }
}

fn maybe_update_target_dir(app: &mut App) {
    if app.here.as_os_str().as_encoded_bytes().len()
        < app
            .read_opts
            .target_dir
            .as_os_str()
            .as_encoded_bytes()
            .len()
    {
        app.read_opts.target_dir.clone_from(&app.here);
    }
}
