use crate::action::{handle_action, item_under_cursor, matches_binding, ActionResult};
use crate::item::Item;
use crate::preview::{run_preview, Preview, PreviewCommand, PreviewedData};
use crate::store::Store;
use crate::tui_log::{LogWidget, LogWidgetState};
use crate::App;
use anyhow::Result;
use crossterm::event;
use crossterm::event::Event;
use log::info;
use nucleo::pattern::{CaseMatching, Normalization};
use nucleo::Snapshot;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use std::collections::VecDeque;
use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use tui_input::backend::crossterm::to_input_request;
use tui_input::Input;

pub struct Ui {
    pub input: Input,
    pub view_start: u32,
    pub cursor: u32,
    pub cursor_showing: PathBuf,
    pub prompt: String,
    pub active: bool,
    pub sorted_items: Vec<u32>,
    pub previews: VecDeque<Preview>,
}

pub fn run(
    store: &mut Store,
    app: &mut App,
    log_state: Arc<Mutex<LogWidgetState>>,
) -> Result<(Option<String>, ExitCode)> {
    let mut terminal = ratatui::try_init()?;
    let _restore = DropRestore {};

    let mut ui = Ui {
        input: Input::default(),
        view_start: 0,
        cursor: 0,
        cursor_showing: app.here.to_path_buf(),
        prompt: format!("{}> ", app.here.display()),
        active: true,
        sorted_items: Vec::new(),
        previews: VecDeque::new(),
    };

    store.start_scan(app)?;

    loop {
        maybe_update_target_dir(app);

        store.nucleo.tick(10);

        let snap = store.nucleo.snapshot();

        ui.active = store.is_scanning() || ui.previews.iter().any(|v| !v.worker.is_finished());

        if ui.active && ui.previews.iter().any(|v| would_flicker(v)) {
            event::poll(Duration::from_millis(60))?;
            thread::yield_now();
        }

        terminal.draw(|f| draw_ui(f, &mut ui, snap, log_state.clone()))?;

        if !event::poll(Duration::from_millis(if ui.active { 5 } else { 90 }))? {
            continue;
        }

        let ev = event::read()?;
        let area = terminal.get_frame().area();

        let binding_action = match ev {
            Event::Key(key) => matches_binding(&app.bindings, key),
            _ => None,
        };

        match binding_action {
            Some(action) => {
                let action = handle_action(action, app, &mut ui, snap)?;
                match action {
                    ActionResult::Ignored => (),
                    ActionResult::Configured => {
                        if let Some(path) =
                            item_under_cursor(&mut ui, snap).and_then(|it| it.path())
                        {
                            ui.cursor_showing = path.to_owned();
                            let mut right_pane_guess = area.clone();
                            right_pane_guess.width /= 2;
                            fire_preview(&mut ui, right_pane_guess);
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

fn would_flicker(v: &Preview) -> bool {
    v.started.elapsed() < Duration::from_millis(100) && !v.worker.is_finished()
}

fn fire_preview(ui: &mut Ui, area: Rect) {
    let started = Instant::now();

    if ui
        .previews
        .iter()
        .any(|v| v.showing == ui.cursor_showing && v.target_area == area)
    {
        return;
    }

    if ui.previews.len() >= 16 {
        ui.previews.pop_front();
    }

    let data = Arc::new(Mutex::new(PreviewedData::default()));

    let write_to = Arc::clone(&data);
    let preview_path = ui.cursor_showing.to_path_buf();
    let worker = thread::spawn(move || {
        if let Err(e) = run_preview(&preview_path, Arc::clone(&write_to), area) {
            write_to
                .lock()
                .expect("panic")
                .content
                .extend_from_slice(format!("Error: {}\n", e).as_bytes());
        }
        info!("preview: {preview_path:?} took {:?}", started.elapsed());
    });

    ui.previews.push_back(Preview {
        showing: ui.cursor_showing.to_path_buf(),
        target_area: area,
        data: Arc::clone(&data),
        worker,
        started,
    });
}

fn draw_ui(
    f: &mut Frame,
    ui: &mut Ui,
    snap: &Snapshot<Item>,
    log_state: Arc<Mutex<LogWidgetState>>,
) {
    let [input_line_area, main_app_area, log_area] = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Percentage(20),
        ])
        .split(f.area())
        .deref()
        .try_into()
        .expect("static constraints");

    let [left_pane_area, divider_area, right_pane_area] = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
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

    if let Ok(log_state) = &mut log_state.lock() {
        f.render_widget(Block::new().borders(Borders::ALL), log_area);
        let log_inset = edge_inset(&log_area, 1);
        f.render_stateful_widget(LogWidget::default(), log_inset, log_state);
    }
}

fn edge_inset(area: &Rect, margin: u16) -> Rect {
    let mut inset_area = *area;
    inset_area.x += margin;
    inset_area.y += margin;
    inset_area.height -= margin;
    inset_area.width -= margin;

    inset_area
}

fn draw_listing(f: &mut Frame, ui: &mut Ui, snap: &Snapshot<Item>, area: Rect) {
    // TODO: not correct; allows positioning one past the end
    ui.cursor = ui.cursor.min(snap.matched_item_count().saturating_sub(1));
    if ui.cursor < ui.view_start {
        ui.view_start = ui.cursor;
    } else if ui.cursor + 1 >= ui.view_start + u32::from(area.height) {
        ui.view_start = ui.cursor.saturating_sub(u32::from(area.height)) + 2;
    }

    let mut lines = Vec::new();
    lines.push(Line::styled(
        format!(
            "{} {}/{}",
            if ui.active { "S" } else { " " },
            snap.matched_item_count(),
            snap.item_count(),
        ),
        Style::new().light_yellow(),
    ));
    let to_show = u32::from(area.height).min(snap.matched_item_count());
    let items = item_range(snap, ui.view_start, ui.view_start + to_show, ui);

    for (i, item) in items.into_iter().enumerate() {
        let mut spans = Vec::new();
        let selected = ui.cursor.saturating_sub(ui.view_start) as usize == i;
        if selected {
            spans.push(Span::styled("> ", Style::new().light_red()));
        } else {
            spans.push(Span::from("  "));
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
            .iter()
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

fn draw_preview(f: &mut Frame, ui: &mut Ui, area: Rect) {
    let preview = match ui.previews.iter().find(|v| v.showing == ui.cursor_showing) {
        Some(preview) => preview,
        None => {
            draw_no_preview(f, area);
            return;
        }
    };

    let data = preview.data.lock().expect("panic");

    match &data.command {
        PreviewCommand::InterpretFile => match data.render.as_ref() {
            Some(rendered) => f.render_widget(rendered, area),
            None => draw_raw_preview(f, area, &preview.showing, "cat", &data.content),
        },
        PreviewCommand::Thinking => draw_raw_preview(f, area, &preview.showing, "file", &[]),
        PreviewCommand::Custom(command) => {
            draw_raw_preview(f, area, &preview.showing, command, &data.content)
        }
    }
}

fn draw_raw_preview(
    f: &mut Frame,
    area: Rect,
    showing: impl AsRef<Path>,
    command: &str,
    content: &[u8],
) {
    let mut lines = vec![Line::from(vec![
        Span::styled(format!("{:>4}", command), Style::new().light_yellow()),
        Span::raw(" "),
        Span::styled(showing.as_ref().display().to_string(), Style::new().bold()),
    ])];

    let cleaned =
        String::from_utf8_lossy(&content).replace(|c: char| c != '\n' && c.is_control(), " ");
    for (i, line) in cleaned
        .split('\n')
        .take(usize::from(area.height))
        .enumerate()
    {
        lines.push(Line::from(Span::raw(format!("{:4} {line}", i + 1))));
    }
    f.render_widget(Text::from(lines), area);
}

fn draw_no_preview(f: &mut Frame, area: Rect) {
    f.render_widget(Paragraph::new("S").wrap(Wrap::default()), area);
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
