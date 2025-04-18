use crate::draw::RightPane::{Hidden, InteractiveGitLog, Preview};
use crate::git::Git;
use crate::git_but_bad::git_log_matches;
use crate::item::{Item, ItemView, Styling, ViewContext};
use crate::preview::{preview_header, PreviewCommand};
use crate::snapped::Snapped;
use crate::tui_log::{LogWidget, LogWidgetState};
use crate::ui_state::{matching_preview, CommandPalette, URect, Ui};
use crate::walk::Recursion;
use crate::{filter_bindings, App, Binding};
use crossterm::event::KeyModifiers;
use ratatui::layout::{Constraint, Direction, Flex, Layout, Rect};
use ratatui::prelude::{Line, Span, Style, Stylize, Text};
use ratatui::style::Color;
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use ratatui::Frame;
use std::ops::Deref;
use std::path::Path;
use std::sync::{Arc, Mutex};
use tui_input::Input;

#[derive(Copy, Clone, PartialEq)]
pub enum RightPane {
    Preview,
    Hidden,
    InteractiveGitLog,
}

#[derive(Copy, Clone, PartialEq)]
pub enum PreviewMode {
    Content,
    GitLg,
    GitShow,
}

pub const RIGHT_PANE: [RightPane; 3] = [Preview, Hidden, InteractiveGitLog];
pub const RIGHT_PANE_HIDDEN: [RightPane; 3] = [Hidden, Preview, InteractiveGitLog];

pub const PREVIEW_MODE: [PreviewMode; 3] = [
    PreviewMode::Content,
    PreviewMode::GitLg,
    PreviewMode::GitShow,
];

#[derive(Copy, Clone)]
pub struct ViewOpts {
    pub right_pane_mode: [RightPane; 3],
    pub preview_mode_flag: [PreviewMode; 3],
    pub log_pane: bool,
    pub git_info: bool,
    pub input_bottom: bool,
}

impl ViewOpts {
    pub fn right_pane(&self) -> RightPane {
        self.right_pane_mode[0]
    }
    pub fn preview_mode(&self) -> PreviewMode {
        self.preview_mode_flag[0]
    }
}

#[derive(Copy, Clone, Debug)]
pub struct Areas {
    pub main_area: Rect,
    pub info_line: Rect,
    pub main_pane: Rect,
    pub side_pane: Rect,
    pub input_line: Rect,
    pub log: Rect,
    pub divider: Rect,
}

impl Areas {
    pub(crate) fn items_required(&self, _view_opts: &ViewOpts) -> u32 {
        u32::from(self.main_pane.height)
    }
}

const STATUS_LINES: usize = 1;

#[derive(Default, Clone)]
struct ColumnEntry<'a> {
    primary: Vec<Span<'a>>,
    short: Vec<Span<'a>>,
    secondary: Vec<Span<'a>>,
    extra: Vec<Span<'a>>,
}

#[derive(Default)]
struct Columns<'a> {
    primary_lines: Vec<Line<'a>>,
    short_lines: Vec<Line<'a>>,
    secondary_lines: Vec<Line<'a>>,
    extra_lines: Vec<Line<'a>>,
}

impl<'a> Columns<'a> {
    fn add(&mut self, entry: ColumnEntry<'a>) {
        self.primary_lines.push(Line::from(entry.primary));
        self.short_lines.push(Line::from(entry.short));
        self.secondary_lines.push(Line::from(entry.secondary));
        self.extra_lines.push(Line::from(entry.extra));
    }
}

pub fn setup_screen(screen: Rect, view_opts: &ViewOpts) -> Areas {
    let log_constraint = if view_opts.log_pane {
        Constraint::Percentage(20)
    } else {
        Constraint::Length(0)
    };

    let [info_line, line_main_top, line_main_bottom, log] = Layout::default()
        .direction(Direction::Vertical)
        .constraints(if view_opts.input_bottom {
            [
                Constraint::Length(1),
                Constraint::Min(0),
                Constraint::Length(1),
                log_constraint,
            ]
        } else {
            [
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Min(0),
                log_constraint,
            ]
        })
        .split(screen)
        .deref()
        .try_into()
        .expect("static constraints");

    let (main_area, input_line) = if view_opts.input_bottom {
        (line_main_top, line_main_bottom)
    } else {
        (line_main_bottom, line_main_top)
    };

    let [main_pane, divider, side_pane] = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(if view_opts.right_pane() != RightPane::Hidden {
            [
                Constraint::Percentage(50),
                Constraint::Length(1),
                Constraint::Percentage(50),
            ]
        } else {
            [
                Constraint::Percentage(100),
                Constraint::Length(1),
                Constraint::Percentage(0),
            ]
        })
        .split(main_area)
        .deref()
        .try_into()
        .expect("static constraints");

    Areas {
        main_area,
        info_line,
        main_pane,
        side_pane,
        input_line,
        log,
        divider,
    }
}

pub fn draw_ui(
    f: &mut Frame,
    area: Areas,
    ui: &Ui,
    app: &App,
    snap: &Snapped,
    log_state: Arc<Mutex<LogWidgetState>>,
) {
    draw_input_line(f, &ui.prompt, &ui.input, area.input_line);
    draw_info_line(f, ui, snap, area.info_line);
    draw_listing(
        f,
        ui,
        snap,
        area.main_pane,
        app.read_opts.recursion != Recursion::None,
    );
    draw_right_pane(f, area, ui, app);

    if ui.command_palette.showing {
        draw_palette(f, &ui.command_palette, &app.bindings, area.main_area);
    }

    if !area.log.is_empty() {
        if let Ok(log_state) = &mut log_state.lock() {
            f.render_widget(Block::new().borders(Borders::ALL), area.log);
            let log_inset = edge_inset(area.log, 1);
            f.render_stateful_widget(LogWidget { boot: ui.boot }, log_inset, log_state);
        }
    }
}

fn draw_right_pane(f: &mut Frame<'_>, area: Areas, ui: &Ui, app: &App) {
    match app.view_opts.right_pane() {
        RightPane::Hidden => (),
        RightPane::Preview => {
            draw_divider(f, area.divider);
            draw_preview(f, ui, app.view_opts.preview_mode(), area.side_pane);
        }
        RightPane::InteractiveGitLog => {
            draw_divider(f, area.divider);
            draw_git_logs(f, ui, area.side_pane);
        }
    }
}

fn draw_palette(f: &mut Frame, palette: &CommandPalette, bindings: &[Binding], area: Rect) {
    let block = Block::bordered().title("palette");
    let area = popup_area(area, 60, 60);
    f.render_widget(Clear, area); //this clears out the background
    f.render_widget(block, area);
    let area = edge_inset(area, 1);
    draw_input_line(f, "> ", &palette.input, area);
    let lines = filter_bindings(bindings, palette.input.value())
        .into_iter()
        .enumerate()
        .map(|(i, (mods, key, action))| {
            Line::raw(format!(
                "{} {:>3}{}{:10} => {}",
                if i == palette.selected { ">" } else { " " },
                render_mods(*mods),
                if mods.is_empty() { " " } else { "+" },
                format!("{key}"),
                action.name()
            ))
        })
        .collect::<Vec<_>>();
    let mut area = area;
    area.y += 1;
    area.height -= 1;
    f.render_widget(Text::from(lines), area);
}

fn render_mods(mods: KeyModifiers) -> String {
    let mut out = String::with_capacity(3);
    if mods.contains(KeyModifiers::CONTROL) {
        out.push('C');
    }
    if mods.contains(KeyModifiers::ALT) {
        out.push('A');
    }
    if mods.contains(KeyModifiers::SHIFT) {
        out.push('S');
    }
    out
}

fn edge_inset(area: Rect, margin: u16) -> Rect {
    let mut inset_area = area;
    inset_area.x += margin;
    inset_area.y += margin;
    inset_area.height -= margin * 2;
    inset_area.width -= margin * 2;

    inset_area
}

fn draw_listing(f: &mut Frame, ui: &Ui, snap: &Snapped, area: Rect, recursive_listing: bool) {
    let mut columns = Columns::default();
    let searching = ui.is_searching();

    let styling = Styling::new(&ui.ls_colors);

    for (i, item) in snap
        .items
        .iter()
        .enumerate()
        .map(|(i, item)| (i as u32 + snap.start, item))
        .take(usize::from(area.height).saturating_sub(STATUS_LINES))
    {
        let rot = compute_rot(searching, i);
        let view = render_item(item, &ui.git_info, &styling, rot);

        let selected = ui.cursor_showing.as_ref() == Some(&item);

        let current_indicator = if selected {
            Span::styled("> ", Style::new().light_red())
        } else {
            Span::raw("  ")
        };

        let current_indicator_right = if selected {
            Span::styled(" <", Style::new().light_red())
        } else {
            Span::raw("  ")
        };

        let mut entry = ColumnEntry::default();

        if recursive_listing {
            entry.short.push(current_indicator.clone());
            entry.short.extend(view.short);

            if view.primary.len() > 0 {
                entry.primary.push(Span::raw("  |"));
                entry.primary.push(current_indicator.clone());
                entry.primary.extend(view.primary);
                entry.primary.push(current_indicator_right);
            }
        } else {
            entry.primary.push(current_indicator.clone());
            entry.primary.extend(view.primary);
            entry.primary.push(current_indicator_right);
        }

        if let Some(secondary) = view.secondary {
            entry.secondary.push(current_indicator.clone());
            entry.secondary.extend(secondary);
        }

        entry.extra = view.annotation;

        if let Some(extra) = view.extra {
            entry.extra.push(current_indicator);
            entry.extra.extend(extra)
        } else {
            entry.extra.push(Span::raw(" "));
        }

        columns.add(entry);
    }

    display_columns(f, area, columns, recursive_listing)
}

fn render_item<'a>(item: &'a Item, git: &Option<Git>, styling: &Styling, rot: f32) -> ItemView<'a> {
    let git_status = item
        .path()
        .and_then(|p| git.as_ref().and_then(|gi| gi.status(p)));

    let git_info = item
        .path()
        .and_then(|p| git.as_ref().and_then(|gi| gi.resolve(p)));

    let context = ViewContext {
        git_status,
        git_info,
        rot,
        styling: &styling,
    };

    item.render(&context)
}

fn display_columns(f: &mut Frame, area: Rect, columns: Columns, with_short: bool) {
    let [short, primary, extra] = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(if with_short { 35 } else { 0 }),
            Constraint::Percentage(70),
            Constraint::Fill(3),
        ])
        .areas(area);

    f.render_widget(Text::from(columns.short_lines), short);
    f.render_widget(Text::from(columns.primary_lines), primary);
    f.render_widget(Text::from(columns.extra_lines), extra);
}

fn compute_rot(searching: bool, i: u32) -> f32 {
    if searching {
        (i as f32 / 30.).min(0.9)
    } else {
        0.
    }
}

fn draw_input_line(f: &mut Frame, prompt: &str, input: &Input, input_line_area: Rect) {
    let mut prompt = Span::styled(prompt, Style::new().light_yellow());
    let mut input_line_remainder = input_line_area.width.saturating_sub(prompt.width() as u16);
    if input_line_remainder < 10 {
        prompt = Span::styled("> ", Style::new().blue());
        input_line_remainder = input_line_area.width.saturating_sub(2);
    }

    let prompt_used = prompt.width() as u16;

    f.render_widget(Paragraph::new(prompt), input_line_area);

    let text_area = Rect::new(
        input_line_area.x + prompt_used,
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

fn draw_info_line(f: &mut Frame, ui: &Ui, snap: &Snapped, area: Rect) {
    let line = Line::styled(
        format!(
            "{}/{} {}",
            snap.matched,
            snap.total,
            if ui.active { "S" } else { " " },
        ),
        Style::new().fg(Color::Indexed(250)),
    );

    f.render_widget(line, area);
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

fn draw_preview(f: &mut Frame, ui: &Ui, mode: PreviewMode, area: Rect) {
    let preview = match matching_preview(ui, mode) {
        Some(preview) => preview,
        None => {
            draw_no_preview(f, area);
            return;
        }
    };

    let data = preview.data.lock().expect("panic");

    let text = match &data.command {
        PreviewCommand::InterpretFile => match data.render.as_ref() {
            Some(rendered) => rendered,
            None => &as_raw_preview(preview.target_area, &preview.showing, "cat", &data.content),
        },
        PreviewCommand::Thinking => {
            &as_raw_preview(preview.target_area, &preview.showing, "file", &[])
        }
        PreviewCommand::Custom(command) => match data.render.as_ref() {
            Some(rendered) => rendered,
            None => &as_raw_preview(
                preview.target_area,
                &preview.showing,
                command,
                &data.content,
            ),
        },
    };

    let allowable_cursor = text.lines.len().saturating_sub(3);

    if ui.preview_cursor > 0 {
        f.render_widget(
            Text::from(
                text.lines
                    .iter()
                    .skip(ui.preview_cursor.min(allowable_cursor))
                    .cloned()
                    .collect::<Vec<_>>(),
            ),
            area,
        )
    } else {
        f.render_widget(text, area);
    }
}

fn as_raw_preview(
    area: URect,
    showing: impl AsRef<Path>,
    command: &str,
    content: &[u8],
) -> Text<'static> {
    let mut lines = vec![preview_header(command, showing)];

    let cleaned =
        String::from_utf8_lossy(content).replace(|c: char| c != '\n' && c.is_control(), " ");
    for (i, line) in cleaned.split('\n').take(area.height).enumerate() {
        lines.push(Line::from(Span::raw(format!("{:4} {line}", i + 1))));
    }
    Text::from(lines)
}

fn draw_no_preview(f: &mut Frame, area: Rect) {
    f.render_widget(Paragraph::new("S").wrap(Wrap::default()), area);
}

fn draw_git_logs(f: &mut Frame, ui: &Ui, area: Rect) {
    let [input, area] =
        Layout::vertical([Constraint::Length(1), Constraint::Percentage(100)]).areas(area);

    if ui.bad_git_log.focus {
        draw_input_line(f, "> ", &ui.bad_git_log.input, input);
    } else {
        f.render_widget(
            Span::styled("  - yo, hit alt+g again to focus me", Color::DarkGray),
            input,
        );
    }

    let mut cache = ui.bad_git_log.cache.borrow_mut();
    let log_data = match ui
        .cursor_showing_path()
        .and_then(|p| cache.get(&p.to_path_buf()))
    {
        Some(v) => v,
        None => return,
    };

    let matches = git_log_matches(log_data, ui.bad_git_log.input.value(), area.height.into());

    // amusingly not necessarily the first (list order) item
    let selected = matches.get(0).copied().unwrap_or_default();

    let lines = log_data
        .entries
        .iter()
        .enumerate()
        .map(|(idx, entry)| {
            let mut spans = entry.as_spans(matches.contains(&idx));
            if selected == idx {
                spans.insert(0, Span::styled("> ", Style::new().light_red()));
            } else {
                spans.insert(0, Span::raw("  "));
            };
            Line::from(spans)
        })
        .collect::<Vec<_>>();

    f.render_widget(Text::from(lines), area);
}

fn popup_area(area: Rect, percent_x: u16, percent_y: u16) -> Rect {
    let vertical = Layout::vertical([Constraint::Percentage(percent_y)]).flex(Flex::Center);
    let horizontal = Layout::horizontal([Constraint::Percentage(percent_x)]).flex(Flex::Center);
    let [area] = vertical.areas(area);
    let [area] = horizontal.areas(area);
    area
}
