use crate::draw::RightPane::{Hidden, Preview, SecondListing};
use crate::item::Styling;
use crate::preview::{preview_header, PreviewCommand};
use crate::snapped::Snapped;
use crate::tui_log::{LogWidget, LogWidgetState};
use crate::ui_state::{matching_preview, URect, Ui};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::prelude::{Line, Span, Style, Stylize, Text};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use ratatui::Frame;
use std::ops::Deref;
use std::path::Path;
use std::sync::{Arc, Mutex};
use tui_input::Input;

#[derive(Copy, Clone, PartialEq)]
pub enum RightPane {
    Preview,
    Hidden,
    SecondListing,
}

pub const RIGHT_PANE: [RightPane; 3] = [Preview, Hidden, SecondListing];

#[derive(Clone)]
pub struct ViewOpts {
    pub right_pane_mode: [RightPane; 3],
}

impl ViewOpts {
    pub fn right_pane(&self) -> RightPane {
        self.right_pane_mode[0]
    }
}

#[derive(Copy, Clone, Debug)]
pub struct Areas {
    pub main_pane: Rect,
    pub side_pane: Rect,
    pub input_line: Rect,
    pub log: Rect,
    pub divider: Rect,
}

pub fn setup_screen(screen: Rect, view_opts: &ViewOpts) -> Areas {
    let [input_line, main_area, log] = Layout::default()
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
    view_opts: &ViewOpts,
    snap: &Snapped,
    log_state: Arc<Mutex<LogWidgetState>>,
) {
    draw_input_line(f, &ui.prompt, &ui.input, area.input_line);

    draw_listing(f, ui, snap, area.main_pane);

    match view_opts.right_pane() {
        RightPane::Hidden => (),
        RightPane::Preview => {
            draw_divider(f, area.divider);
            draw_preview(f, ui, area.side_pane);
        }
        RightPane::SecondListing => {
            draw_divider(f, area.divider);
            draw_second_listing(f, ui, snap, area.side_pane);
        }
    }

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

const STATUS_LINES: usize = 1;

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

    assert_eq!(lines.len(), STATUS_LINES);

    let searching = ui.is_searching();

    let styling = Styling::new(&ui.ls_colors);

    for (i, item) in snap
        .items
        .iter()
        .take(usize::from(area.height).saturating_sub(STATUS_LINES))
        .copied()
    {
        let selected = ui.cursor == i;
        let rot = compute_rot(searching, i);

        let mut spans = Vec::new();
        if selected {
            spans.push(Span::styled("> ", Style::new().light_red()));
        } else {
            spans.push(Span::from("  "));
        }

        spans.extend(item.as_spans(&styling, rot));
        lines.push(Line::from(spans));
    }
    f.render_widget(Text::from(lines), area);
}

fn compute_rot(searching: bool, i: u32) -> f32 {
    if searching {
        (i as f32 / 30.).min(0.9)
    } else {
        0.
    }
}

fn draw_second_listing(f: &mut Frame, ui: &Ui, snap: &Snapped, area: Rect) {
    let mut lines = Vec::new();

    let searching = ui.is_searching();

    let styling = Styling::new(&ui.ls_colors);

    for (i, item) in snap
        .items
        .iter()
        .skip(usize::from(area.height).saturating_sub(STATUS_LINES))
        .copied()
    {
        let selected = ui.cursor == i;
        let rot = compute_rot(searching, i);

        let mut spans = Vec::new();
        if selected {
            spans.push(Span::styled("> ", Style::new().light_red()));
        } else {
            spans.push(Span::from("  "));
        }

        spans.extend(item.as_spans(&styling, rot));
        lines.push(Line::from(spans));
    }
    f.render_widget(Text::from(lines), area);
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
    let preview = match matching_preview(ui) {
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
