use crate::action::{handle_action, item_under_cursor, matches_binding, ActionResult};
use crate::item::Item;
use crate::preview::{preview_header, run_preview, Preview, PreviewCommand, PreviewedData};
use crate::store::Store;
use crate::tui_log::{LogWidget, LogWidgetState};
use crate::App;
use anyhow::Result;
use crossterm::event::Event;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::{event, execute};
use log::info;
use lscolors::LsColors;
use nucleo::pattern::{CaseMatching, Normalization};
use nucleo::Snapshot;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use std::collections::VecDeque;
use std::io::stderr;
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
    pub cursor_showing: Option<PathBuf>,
    pub prompt: String,
    pub active: bool,
    pub sorted_items: Vec<u32>,
    pub sorted_until: usize,
    pub previews: VecDeque<Preview>,
    pub preview_colours: bool,
    pub ls_colors: LsColors,
}

pub fn run(
    store: &mut Store,
    app: &mut App,
    log_state: Arc<Mutex<LogWidgetState>>,
) -> Result<(Option<String>, ExitCode)> {
    // copy-paste of ratatui::try_init() but for stderr
    enable_raw_mode()?;
    execute!(stderr(), EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stderr());
    let mut terminal = Terminal::new(backend)?;
    let _restore = DropRestore {};

    let mut ui = Ui {
        input: Input::default(),
        view_start: 0,
        cursor: 0,
        cursor_showing: Some(app.here.to_path_buf()),
        prompt: format!("{}> ", app.here.display()),
        active: true,
        sorted_items: Vec::new(),
        sorted_until: 0,
        previews: VecDeque::new(),
        preview_colours: true,
        ls_colors: LsColors::from_env().unwrap_or_default(),
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

        let last_area = terminal
            .draw(|f| {
                let area = setup_screen(f.area());
                fire_preview(&mut ui, area.right_pane);
                let item_area = area.left_pane;
                revalidate_cursor(&mut ui, snap, item_area);
                let items = ui_item_range(&mut ui, snap, item_area);
                draw_ui(f, area, &ui, &items, log_state.clone())
            })?
            .area;

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
                let action = handle_action(action, app, &mut ui)?;
                match action {
                    ActionResult::Ignored => (),
                    ActionResult::Configured => {
                        ui.cursor_showing = item_under_cursor(&mut ui, snap).map(PathBuf::from);
                        fire_preview(&mut ui, setup_screen(last_area).right_pane);
                    }

                    ActionResult::Navigated => {
                        app.read_opts.expansions.clear();
                        ui.prompt = format!("{}> ", app.here.display());
                        ui.sorted_items.clear();
                        ui.sorted_until = 0;
                        store.start_scan(app)?;
                    }

                    ActionResult::JustRescan => {
                        ui.sorted_items.clear();
                        ui.sorted_until = 0;
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

fn ui_item_range<'s>(ui: &mut Ui, snap: &'s Snapshot<Item>, item_area: Rect) -> Snapped<'s> {
    item_range(
        snap,
        ui.view_start,
        ui.view_start.saturating_add(u32::from(item_area.height)),
        ui,
    )
}

fn would_flicker(v: &Preview) -> bool {
    v.started.elapsed() < Duration::from_millis(100) && !v.worker.is_finished()
}

fn revalidate_cursor(ui: &mut Ui, snap: &Snapshot<Item>, area: Rect) {
    ui.cursor = ui.cursor.min(snap.matched_item_count().saturating_sub(1));
    ui.cursor_showing = item_under_cursor(ui, snap).map(PathBuf::from);

    if ui.cursor < ui.view_start {
        ui.view_start = ui.cursor;
    } else if ui.cursor + 1 >= ui.view_start + u32::from(area.height) {
        ui.view_start = ui.cursor.saturating_sub(u32::from(area.height)) + 2;
    }
}

fn fire_preview(ui: &mut Ui, preview_area: Rect) {
    if preview_area.width == 0 || preview_area.height == 0 {
        return;
    }

    let showing = match ui.cursor_showing {
        Some(ref v) => v,
        None => return,
    };

    let started = Instant::now();

    if ui.previews.iter().rev().any(|v| {
        Some(&v.showing) == ui.cursor_showing.as_ref()
            && v.target_area == preview_area
            && v.coloured == ui.preview_colours
    }) {
        return;
    }

    if ui.previews.len() >= 16 {
        ui.previews.pop_front();
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

    ui.previews.push_back(Preview {
        showing: showing.to_path_buf(),
        target_area: preview_area,
        coloured: ui.preview_colours,
        data: Arc::clone(&data),
        worker,
        started,
    });
}

#[derive(Copy, Clone, Debug)]
struct Areas {
    input_line: Rect,
    log: Rect,
    left_pane: Rect,
    divider: Rect,
    right_pane: Rect,
}

fn setup_screen(screen: Rect) -> Areas {
    let [input_line, main_app, log] = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Percentage(20),
        ])
        .split(screen)
        .deref()
        .try_into()
        .expect("static constraints");

    let [left_pane, divider, right_pane] = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(50),
            Constraint::Length(1),
            Constraint::Percentage(50),
        ])
        .split(main_app)
        .deref()
        .try_into()
        .expect("static constraints");

    Areas {
        input_line,
        log,
        left_pane,
        divider,
        right_pane,
    }
}

fn draw_ui(
    f: &mut Frame,
    area: Areas,
    ui: &Ui,
    snap: &Snapped,
    log_state: Arc<Mutex<LogWidgetState>>,
) {
    draw_input_line(f, &ui.prompt, &ui.input, area.input_line);

    draw_listing(f, ui, snap, area.left_pane);
    draw_divider(f, area.divider);
    draw_preview(f, ui, area.right_pane);

    if let Ok(log_state) = &mut log_state.lock() {
        f.render_widget(Block::new().borders(Borders::ALL), area.log);
        let log_inset = edge_inset(area.log, 1);
        f.render_stateful_widget(LogWidget::default(), log_inset, log_state);
    }
}

fn edge_inset(area: Rect, margin: u16) -> Rect {
    let mut inset_area = area;
    inset_area.x += margin;
    inset_area.y += margin;
    inset_area.height -= margin;
    inset_area.width -= margin;

    inset_area
}

fn draw_listing(f: &mut Frame, ui: &Ui, snap: &Snapped, area: Rect) {
    let mut lines = Vec::new();
    lines.push(Line::styled(
        format!(
            "{} {}/{}",
            if ui.active { "S" } else { " " },
            snap.matched,
            snap.total,
        ),
        Style::new().light_yellow(),
    ));

    let searching = !ui.input.value().is_empty();

    for (i, item) in snap.items.iter().enumerate() {
        let mut spans = Vec::new();
        let selected = ui.cursor.saturating_sub(ui.view_start) as usize == i;
        if selected {
            spans.push(Span::styled("> ", Style::new().light_red()));
        } else {
            spans.push(Span::from("  "));
        }

        let overall_pos = (ui.view_start as usize).saturating_add(i);
        spans.extend(item.as_spans(
            &ui.ls_colors,
            if searching {
                (overall_pos as f32 / 30.).min(0.9)
            } else {
                0.
            },
        ));
        lines.push(Line::from(spans));
    }
    f.render_widget(Text::from(lines), area);
}

pub struct Snapped<'i> {
    pub items: Vec<&'i Item>,
    pub matched: u32,
    pub total: u32,
}

#[inline]
pub fn item_range<'s>(
    snap: &'s Snapshot<Item>,
    start: u32,
    mut end: u32,
    ui: &mut Ui,
) -> Snapped<'s> {
    if end > snap.matched_item_count() {
        end = snap.matched_item_count();
    }
    if start >= end {
        return Snapped {
            items: Vec::new(),
            matched: snap.matched_item_count(),
            total: snap.item_count(),
        };
    }

    let sort = ui.input.value().is_empty();
    let items = if !sort {
        snap.matched_items(start..end)
            .map(|item| item.data)
            .collect()
    } else {
        let real_end = snap.matched_item_count();
        let cache_end = ui.sorted_items.len() as u32;
        let could_extend = real_end > cache_end;
        let should_extend = end * 2 > cache_end || real_end % 64 == 0;
        let should_sort = end as usize > ui.sorted_until;
        if should_sort || (could_extend && should_extend) {
            ui.sorted_items.extend(cache_end..real_end);

            if end < real_end {
                ui.sorted_items
                    .select_nth_unstable_by_key(end as usize, |&i| {
                        snap.get_item(i).expect("<end").data
                    });
            }

            ui.sorted_items[0..end as usize]
                .sort_unstable_by_key(|&i| snap.get_item(i).expect("<end").data);
            ui.sorted_until = end as usize;
        }

        ui.sorted_items[start as usize..end as usize]
            .iter()
            .map(|&i| snap.get_item(i).expect("<end").data)
            .collect()
    };

    Snapped {
        items,
        matched: snap.matched_item_count(),
        total: snap.item_count(),
    }
}

fn draw_input_line(f: &mut Frame, prompt: &str, input: &Input, input_line_area: Rect) {
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

fn draw_preview(f: &mut Frame, ui: &Ui, area: Rect) {
    let preview = match ui.previews.iter().rev().find(|v| {
        Some(&v.showing) == ui.cursor_showing.as_ref() && v.coloured == ui.preview_colours
    }) {
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
        PreviewCommand::Custom(command) => match data.render.as_ref() {
            Some(rendered) => f.render_widget(rendered, area),
            None => draw_raw_preview(f, area, &preview.showing, command, &data.content),
        },
    }
}

fn draw_raw_preview(
    f: &mut Frame,
    area: Rect,
    showing: impl AsRef<Path>,
    command: &str,
    content: &[u8],
) {
    let mut lines = vec![preview_header(command, showing)];

    let cleaned =
        String::from_utf8_lossy(content).replace(|c: char| c != '\n' && c.is_control(), " ");
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
        // copy-paste of ratatui::restore() but for stderr
        let _ = disable_raw_mode();
        let _ = execute!(stderr(), LeaveAlternateScreen);
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
