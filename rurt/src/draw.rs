use crate::item::Styling;
use crate::preview::{preview_header, PreviewCommand};
use crate::snapped::Snapped;
use crate::tui_log::{LogWidget, LogWidgetState};
use crate::ui_state::Ui;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::prelude::{Line, Span, Style, Stylize, Text};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use ratatui::Frame;
use std::ops::Deref;
use std::path::Path;
use std::sync::{Arc, Mutex};
use tui_input::Input;

#[derive(Copy, Clone, Debug)]
pub struct Areas {
    pub input_line: Rect,
    pub log: Rect,
    pub left_pane: Rect,
    pub divider: Rect,
    pub right_pane: Rect,
}

pub fn setup_screen(screen: Rect) -> Areas {
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

pub fn draw_ui(
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

    let styling = Styling::new(&ui.ls_colors);

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
            &styling,
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
