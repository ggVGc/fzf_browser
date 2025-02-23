use crate::action::{handle_action, item_under_cursor, matches_binding, ActionResult};
use crate::item::Item;
use crate::preview::{run_preview, Preview, PreviewedData};
use crate::store::Store;
use crate::tui_log::{LogWidget, LogWidgetState};
use crate::App;
use anyhow::Result;
use content_inspector::ContentType;
use crossterm::event;
use crossterm::event::Event;
use log::info;
use nucleo::pattern::{CaseMatching, Normalization};
use nucleo::Snapshot;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use std::io;
use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;
use tui_input::backend::crossterm::to_input_request;
use tui_input::Input;

pub struct Ui {
    pub input: Input,
    pub view_start: u32,
    pub cursor: u32,
    pub prompt: String,
    pub active: bool,
    pub sorted_items: Vec<u32>,
    pub preview: Preview,
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
        prompt: format!("{}> ", app.here.display()),
        active: true,
        sorted_items: Vec::new(),
        preview: Preview {
            showing: PathBuf::new(),
            content: Arc::new(Mutex::new(PreviewedData::default())),
            worker: None,
        },
    };

    store.start_scan(app)?;

    loop {
        maybe_update_target_dir(app);

        store.nucleo.tick(10);

        let snap = store.nucleo.snapshot();

        ui.active = store.is_scanning() || still_running(ui.preview.worker.as_ref());

        terminal.draw(|f| draw_ui(f, &mut ui, snap, log_state.clone()))?;

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
                let action = handle_action(action, app, &mut ui, snap)?;
                match action {
                    ActionResult::Ignored => (),
                    ActionResult::Configured => {
                        if let Some(path) =
                            item_under_cursor(&mut ui, snap).and_then(|it| it.path())
                        {
                            info!("path: {:?}", path);
                            open_preview(&mut ui, path);
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

pub fn still_running(maybe_handle: Option<&JoinHandle<()>>) -> bool {
    maybe_handle.map(|w| !w.is_finished()).unwrap_or(false)
}

fn open_preview(ui: &mut Ui, path: &Path) {
    ui.preview.showing = path.to_owned();
    ui.preview.content = Arc::new(Mutex::new(PreviewedData::default()));
    let write_to = Arc::clone(&ui.preview.content);
    let path = path.to_owned();
    ui.preview.worker = Some(std::thread::spawn(move || {
        if let Err(e) = run_preview(&path, Arc::clone(&write_to)) {
            write_to
                .lock()
                .expect("panic")
                .content
                .extend_from_slice(format!("Error: {}\n", e).as_bytes());
        }
    }));
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
        ui.view_start = ui.cursor - u32::from(area.height) + 2;
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

fn draw_preview(f: &mut Frame, ui: &mut Ui, right_pane_area: Rect) {
    let preview = ui.preview.content.lock().expect("panic");
    if preview.command.is_empty() {
        f.render_widget(
            Paragraph::new("Select something to preview something else.").wrap(Wrap::default()),
            right_pane_area,
        );
        return;
    }

    let mut lines = vec![
        Line::from(vec![
            Span::styled(&preview.command, Style::new().light_yellow()),
            Span::raw(" "),
            Span::styled(
                ui.preview.showing.display().to_string(),
                Style::new().light_green(),
            ),
        ]),
        Line::from(""),
    ];

    if preview.command == "cat" {
        use ansi_to_tui::IntoText as _;

        let s = match content_inspector::inspect(&preview.content) {
            ContentType::BINARY => {
                let mut v = Vec::new();
                let panels = (right_pane_area.width.saturating_sub(10) / 35).max(1);
                hexyl::PrinterBuilder::new(&mut v)
                    .num_panels(panels as u64)
                    .build()
                    .print_all(io::Cursor::new(&preview.content))
                    .expect("hexylation");
                v.into_text()
            }
            _ => {
                let mut s = String::new();
                bat::PrettyPrinter::new()
                    .input(bat::Input::from_bytes(&preview.content).name(&ui.preview.showing))
                    .header(true)
                    .term_width(right_pane_area.width as usize)
                    .tab_width(Some(2))
                    .line_numbers(true)
                    .use_italics(false)
                    .print_with_writer(Some(&mut s))
                    .expect("infalliable writer?");
                s.into_text()
            }
        };

        f.render_widget(s.expect("valid ansi from libraries"), right_pane_area);
        return;
    }

    let cleaned = String::from_utf8_lossy(&preview.content)
        .replace(|c: char| c != '\n' && c.is_control(), " ");
    for line in cleaned.split('\n') {
        lines.push(Line::from(Span::raw(line)));
    }
    f.render_widget(Text::from(lines), right_pane_area);
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
