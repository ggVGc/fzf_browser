use std::path::PathBuf;

use crossterm::event::{KeyCode, KeyModifiers};

use crate::action::Action;
use crate::dir_stack::DirStack;
use crate::walk::ReadOpts;

pub mod action;
pub mod dir_stack;
pub mod fuzz;
pub mod item;
mod preview;
pub mod ratui;
pub mod store;
pub mod tui_log;
pub mod walk;

pub struct App {
    pub here: PathBuf,
    pub dir_stack: DirStack<PathBuf>,
    pub read_opts: ReadOpts,
    pub bindings: Vec<(KeyModifiers, KeyCode, Action)>,
}
