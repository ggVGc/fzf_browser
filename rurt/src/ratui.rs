use crate::item::Item;
use anyhow::Result;
use crossterm::event;
use crossterm::event::{Event, KeyCode, KeyEvent};
use nucleo::pattern::{CaseMatching, Normalization};
use nucleo::Nucleo;
use ratatui::prelude::*;
use ratatui::widgets::Paragraph;
use std::ops::Deref;
use std::time::Duration;
use tui_input::backend::crossterm::to_input_request;
use tui_input::Input;

struct Ui {
    input: Input,
    cursor: u32,
}

pub fn run(
    nucleo: &mut Nucleo<Item>,
    prompt: impl AsRef<str>,
) -> Result<(KeyEvent, Option<&Item>)> {
    let prompt = prompt.as_ref();
    let mut terminal = ratatui::try_init()?;
    let _restore = DropRestore {};

    let mut ui = Ui {
        input: Input::default(),
        cursor: 0,
    };

    loop {
        terminal.draw(|f| {
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

            draw_input_line(f, prompt, &mut ui.input, input_line_area);

            draw_listing(f, &mut ui, nucleo, left_pane_area);
            draw_divider(f, divider_area);
            draw_preview(f, right_pane_area);
        })?;

        nucleo.tick(10);

        if !event::poll(Duration::from_millis(6))? {
            continue;
        }

        let ev = event::read()?;
        if let Some(req) = to_input_request(&ev) {
            if ui.input.handle(req).map(|v| v.value).unwrap_or_default() {
                nucleo.pattern.reparse(
                    0,
                    ui.input.value(),
                    CaseMatching::Ignore,
                    Normalization::Smart,
                    false,
                );
            }
            continue;
        }

        match ev {
            Event::Key(key) if key.code == KeyCode::Up => {
                ui.cursor = ui.cursor.saturating_sub(1);
            }
            Event::Key(key) if key.code == KeyCode::Down => {
                ui.cursor = ui.cursor.saturating_add(1);
            }
            Event::Key(key) => {
                let snap = nucleo.snapshot();

                let item = snap
                    .get_matched_item(ui.cursor.min(snap.matched_item_count()))
                    .map(|item| item.data);
                return Ok((key, item));
            }
            _ => (),
        }
    }
}

fn draw_listing(f: &mut Frame, ui: &mut Ui, nucleo: &mut Nucleo<Item>, area: Rect) {
    let snap = nucleo.snapshot();

    // TODO: not correct; allows positioning one past the end
    ui.cursor = ui.cursor.min(snap.matched_item_count());

    let mut lines = Vec::new();
    lines.push(Line::styled(
        format!("{}/{}", snap.matched_item_count(), snap.item_count()),
        Style::new().light_yellow(),
    ));
    let to_show = u32::from(area.height).min(snap.matched_item_count());
    let mut items = snap
        .matched_items(0..to_show)
        .map(|item| item.data)
        .collect::<Vec<_>>();
    if ui.input.value().is_empty() {
        items.sort_unstable();
    }

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

fn draw_preview(f: &mut Frame, right_pane_area: Rect) {
    f.render_widget(
        Paragraph::new("this is where the preview would be\nif we had one"),
        right_pane_area,
    );
}

struct DropRestore {}
impl Drop for DropRestore {
    fn drop(&mut self) {
        ratatui::restore();
    }
}
