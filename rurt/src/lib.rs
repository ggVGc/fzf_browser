use crate::dir_stack::DirStack;
use crate::walk::ReadOpts;
use std::path::PathBuf;

pub mod dir_stack;
pub mod fuzz;
pub mod item;
pub mod ratui;
pub mod store;
pub mod walk;

pub struct App {
    pub here: PathBuf,
    pub dir_stack: DirStack<PathBuf>,
    pub read_opts: ReadOpts,
}
