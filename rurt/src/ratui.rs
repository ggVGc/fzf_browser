use crate::item::Item;
use anyhow::Result;
use crossterm::event;
use crossterm::event::{Event, KeyCode, KeyEvent};
use nucleo::pattern::{CaseMatching, Normalization};
use nucleo::Nucleo;
use ratatui::prelude::*;
use ratatui::widgets::Paragraph;
use std::time::Duration;
use tui_input::backend::crossterm::to_input_request;
use tui_input::Input;

pub fn run(nucleo: &mut Nucleo<Item>) -> Result<(KeyEvent, Option<&Item>)> {
    let mut terminal = ratatui::try_init()?;
    let _restore = DropRestore {};

    let mut input = Input::default();
    let mut cursor = 0u32;

    loop {
        terminal.draw(|f| {
            let upper = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(1), Constraint::Min(0)].as_ref())
                .split(f.area());

            let text_area = upper[0];
            let main_app_area = upper[1];

            let panes = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
                .split(main_app_area);

            let left_pane_area = panes[0];
            let right_pane_area = panes[1];

            let scroll = input.visual_scroll(usize::from(text_area.width));
            f.render_widget(
                Paragraph::new(input.value()).scroll((0, scroll as u16)),
                text_area,
            );
            f.set_cursor_position((
                text_area.x + (input.visual_cursor().max(scroll) - scroll) as u16,
                text_area.y,
            ));

            let snap = nucleo.snapshot();

            // TODO: not correct; allows positioning one past the end
            cursor = cursor.min(snap.matched_item_count());

            let mut lines = Vec::new();
            lines.push(Line::from(format!(
                "{}/{}",
                snap.matched_item_count(),
                snap.item_count()
            )));
            let to_show = u32::from(left_pane_area.height).min(snap.matched_item_count());
            let mut items = snap
                .matched_items(0..to_show)
                .map(|item| item.data)
                .collect::<Vec<_>>();
            if input.value().is_empty() {
                items.sort_unstable();
            }

            for (i, item) in items.into_iter().enumerate() {
                lines.push(Line::from(vec![
                    Span::from(if cursor as usize == i { "> " } else { "  " }),
                    item.as_span(),
                ]));
            }
            f.render_widget(Text::from(lines), left_pane_area);

            f.render_widget(
                Paragraph::new("this is where the preview would be\nif we had one"),
                right_pane_area,
            );
        })?;

        nucleo.tick(10);

        if !event::poll(Duration::from_millis(6))? {
            continue;
        }

        let ev = event::read()?;
        if let Some(req) = to_input_request(&ev) {
            if input.handle(req).map(|v| v.value).unwrap_or_default() {
                nucleo.pattern.reparse(
                    0,
                    input.value(),
                    CaseMatching::Ignore,
                    Normalization::Smart,
                    false,
                );
            }
            continue;
        }

        match ev {
            Event::Key(key) if key.code == KeyCode::Up => {
                cursor = cursor.saturating_sub(1);
            }
            Event::Key(key) if key.code == KeyCode::Down => {
                cursor = cursor.saturating_add(1);
            }
            Event::Key(key) => {
                let snap = nucleo.snapshot();

                let item = snap
                    .get_matched_item(cursor.min(snap.matched_item_count()))
                    .map(|item| item.data);
                return Ok((key, item));
            }
            _ => (),
        }
    }
}

struct DropRestore {}
impl Drop for DropRestore {
    fn drop(&mut self) {
        ratatui::restore();
    }
}
