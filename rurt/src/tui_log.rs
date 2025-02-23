use log::{set_max_level, Level, LevelFilter, Log, Metadata, Record, SetLoggerError};
use ratatui::prelude::*;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    widgets::{StatefulWidget, Widget},
};
use std::sync::{Arc, Mutex};

pub trait Writable {
    fn write_line(&mut self, level: Level, message: &str);
    fn flush(&mut self);
}

pub struct TuiLogger<W: Writable + Send + 'static> {
    level: LevelFilter,
    writable: Arc<Mutex<W>>,
}

impl<W: Writable + Send + 'static> TuiLogger<W> {
    pub fn init(log_level: LevelFilter, writable: Arc<Mutex<W>>) -> Result<(), SetLoggerError> {
        set_max_level(log_level);
        log::set_boxed_logger(TuiLogger::new(log_level, writable))
    }

    pub fn new(log_level: LevelFilter, writable: Arc<Mutex<W>>) -> Box<TuiLogger<W>> {
        Box::new(TuiLogger {
            level: log_level,
            writable,
        })
    }
}

impl<W: Writable + Send + 'static> Log for TuiLogger<W> {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        metadata.level() <= self.level
    }

    fn log(&self, record: &Record<'_>) {
        if self.enabled(record.metadata()) {
            let target = if !record.target().is_empty() {
                record.target()
            } else {
                record.module_path().unwrap_or_default()
            };

            let mut write_lock = self.writable.lock().unwrap();

            write_lock.write_line(
                record.level(),
                format!("{:<5}: [{}] {}", record.level(), target, record.args()).as_str(),
            );
        }
    }

    fn flush(&self) {
        self.writable.lock().unwrap().flush();
    }
}

#[derive(Clone)]
pub struct HistoryEntry {
    level: Level,
    text: String,
}

#[derive(Default, Clone)]
pub struct LogWidgetState {
    pub history: Vec<HistoryEntry>,
}

#[derive(Default, Clone)]
pub struct LogWidget {}

impl StatefulWidget for LogWidget {
    type State = LogWidgetState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let max_lines = area.height - 1;

        let history_to_show = state.history.iter().rev().take(max_lines as usize).rev();

        for (y, entry) in history_to_show.enumerate() {
            let mut style = Style::default();
            if entry.level == Level::Error {
                style.fg = Some(Color::Red);
            }

            buf.set_string(
                area.left(),
                area.top() + y as u16,
                entry.text.clone(),
                style,
            );
        }
    }
}

impl Widget for LogWidget {
    fn render(self, area: Rect, buf: &mut Buffer) {
        StatefulWidget::render(self, area, buf, &mut LogWidgetState::default())
    }
}

impl Writable for LogWidgetState {
    fn write_line(&mut self, level: Level, message: &str) {
        self.history.push(HistoryEntry {
            level,
            text: message.to_string(),
        })
    }

    fn flush(&mut self) {
        self.history.clear()
    }
}
