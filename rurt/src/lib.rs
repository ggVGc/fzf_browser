use std::path::PathBuf;

use crossterm::event::{KeyCode, KeyModifiers};

use crate::action::Action;
use crate::dir_stack::DirStack;
use crate::walk::ReadOpts;
use draw::ViewOpts;

pub mod action;
mod colour;
pub mod dir_stack;
pub mod draw;
pub mod fuzz;
pub mod item;
mod line_stop;
mod preview;
pub mod ratui;
mod snapped;
pub mod store;
pub mod tui_log;
mod ui_state;
pub mod walk;

pub struct App {
    pub here: PathBuf,
    pub dir_stack: DirStack<PathBuf>,
    pub read_opts: ReadOpts,
    pub view_opts: ViewOpts,
    pub bindings: Vec<(KeyModifiers, KeyCode, Action)>,
}
