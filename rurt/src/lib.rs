use std::path::PathBuf;

use crate::action::Action;
use crate::dir_stack::DirStack;
use crate::git::Git;
use crate::walk::ReadOpts;
use crossterm::event::{KeyCode, KeyModifiers};
use draw::ViewOpts;
use nucleo::pattern::{CaseMatching, Normalization, Pattern};
use nucleo::{Config, Matcher};

pub mod action;
mod alt_screen;
mod cache;
mod colour;
pub mod dir_stack;
pub mod draw;
pub mod fuzz;
mod git;
mod git_but_bad;
pub mod item;
mod line_stop;
mod preview;
pub mod ratui;
mod snapped;
pub mod store;
pub mod tui_log;
mod ui_state;
pub mod walk;

#[derive(Copy, Clone)]
pub struct ResultOpts {
    pub force_absolute_path: bool,
}

pub type Binding = (KeyModifiers, KeyCode, Action);

pub struct App {
    pub here: PathBuf,
    pub dir_stack: DirStack<PathBuf>,
    pub read_opts: ReadOpts,
    pub view_opts: ViewOpts,
    pub result_opts: ResultOpts,
    pub bindings: Vec<Binding>,
}

impl App {
    fn git_info(&self) -> Option<Git> {
        self.view_opts
            .git_info
            .then(|| Git::new(&self.here))
            .flatten()
    }
}

// couldn't get this to borrow check with inner.name()
struct BindingWrap<'b> {
    inner: &'b Binding,
    name: String,
}

impl<'b> BindingWrap<'b> {
    fn new(inner: &'b Binding) -> Self {
        Self {
            name: inner.2.name().to_string(),
            inner,
        }
    }
}

impl<'b> AsRef<str> for BindingWrap<'b> {
    fn as_ref(&self) -> &str {
        self.name.as_ref()
    }
}

pub fn filter_bindings<'b>(bindings: &'b [Binding], search: &str) -> Vec<&'b Binding> {
    let mut matcher = Matcher::new(Config::DEFAULT);
    let pattern = Pattern::parse(search, CaseMatching::Smart, Normalization::Smart);
    pattern
        .match_list(bindings.iter().map(BindingWrap::new), &mut matcher)
        .into_iter()
        .map(|(m, _)| m.inner)
        .collect()
}
