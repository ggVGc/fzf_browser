use std::path::PathBuf;

use crossterm::event::{KeyCode, KeyModifiers};

use crate::action::Action;
use crate::dir_stack::DirStack;
use crate::git::Git;
use crate::walk::ReadOpts;
use draw::ViewOpts;

pub mod action;
mod alt_screen;
mod cache;
mod colour;
pub mod dir_stack;
pub mod draw;
pub mod fuzz;
mod git;
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

pub struct App {
    pub here: PathBuf,
    pub dir_stack: DirStack<PathBuf>,
    pub read_opts: ReadOpts,
    pub view_opts: ViewOpts,
    pub result_opts: ResultOpts,
    pub bindings: Vec<(KeyModifiers, KeyCode, Action)>,
}

impl App {
    fn git_info(&self) -> Option<Git> {
        self.view_opts
            .git_info
            .then(|| Git::new(&self.here))
            .flatten()
    }
}
