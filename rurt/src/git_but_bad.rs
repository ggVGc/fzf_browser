use crate::cache::Cache;
use anyhow::Result;
use nucleo::pattern::{CaseMatching, Normalization, Pattern};
use nucleo::{Config, Matcher};
use ratatui::style::Color;
use ratatui::text::Span;
use std::cell::RefCell;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tui_input::Input;

#[derive(Default)]
pub struct LogEntry {
    pub hash: String,
    pub rel_date: String,
    pub decorate: String,
    pub author: String,
    pub subject: String,
}

impl LogEntry {
    pub fn as_spans(&self, matching: bool) -> Vec<Span> {
        let mut spans = Vec::new();

        let wm = |s: Color| {
            if matching {
                s
            } else {
                Color::DarkGray
            }
        };

        spans.extend(vec![
            Span::raw("* "),
            Span::styled(&self.hash, wm(Color::Red)),
            Span::raw(" - "),
        ]);
        if !self.decorate.is_empty() {
            spans.push(Span::styled(
                format!("({}) ", &self.decorate),
                wm(Color::Yellow),
            ));
        }
        spans.extend(vec![
            Span::styled(&self.subject, wm(Color::White)),
            Span::styled(format!(" ({})", self.rel_date), wm(Color::Green)),
            Span::styled(format!(" <{}>", self.author), wm(Color::Blue)),
        ]);

        spans
    }
}

pub fn bad_log(path: impl AsRef<Path>, max_count: usize) -> Result<Vec<LogEntry>> {
    let path = path.as_ref();
    let parent = path.parent().ok_or_else(|| anyhow::anyhow!("no parent"))?;
    let output = std::process::Command::new("git")
        .args([
            OsStr::new("log"),
            OsStr::new("--format=%h%x00%ad%x00%D%x00%an%x00%s"),
            OsStr::new("--date=relative"),
            OsStr::new("-n"),
            OsStr::new(&max_count.to_string()),
            path.as_os_str(),
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .current_dir(parent)
        .output()?;

    let mut entries = Vec::with_capacity(max_count);

    for line in output.stdout.split(|&c| c == b'\n') {
        if line.is_empty() {
            continue;
        }
        let mut log_entry = LogEntry::default();
        for (i, field) in line.split(|&c| c == b'\0').enumerate() {
            match i {
                0 => log_entry.hash = String::from_utf8_lossy(field).to_string(),
                1 => log_entry.rel_date = String::from_utf8_lossy(field).to_string(),
                2 => log_entry.decorate = String::from_utf8_lossy(field).to_string(),
                3 => log_entry.author = String::from_utf8_lossy(field).to_string(),
                4 => log_entry.subject = String::from_utf8_lossy(field).to_string(),
                _ => unreachable!("unexpected field"),
            }
        }
        entries.push(log_entry);
    }

    Ok(entries)
}

pub fn git_log_matches(log_data: &LogData, input: &str, limit: usize) -> Vec<usize> {
    struct LogEntryWrap<'l>((usize, &'l LogEntry));

    impl AsRef<str> for LogEntryWrap<'_> {
        fn as_ref(&self) -> &str {
            self.0 .1.subject.as_str()
        }
    }

    let mut matcher = Matcher::new(Config::DEFAULT);
    let pattern = Pattern::parse(input, CaseMatching::Smart, Normalization::Smart);

    pattern
        .match_list(
            log_data
                .entries
                .iter()
                .take(limit)
                .enumerate()
                .map(LogEntryWrap),
            &mut matcher,
        )
        .into_iter()
        .map(|(m, _)| m.0 .0)
        .collect()
}

#[derive(Default)]
pub struct Logs {
    pub cache: RefCell<Cache<PathBuf, LogData>>,
    pub focus: bool,
    pub input: Input,
}

#[derive(Default)]
pub struct LogData {
    pub entries: Vec<LogEntry>,
}
