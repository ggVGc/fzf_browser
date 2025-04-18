use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::fuzz::AddItem;
use crate::item::{convert, Item};

use anyhow::Result;
use ignore::{DirEntry, Error as DirEntryError, WalkBuilder, WalkState};

pub type DResult = Result<DirEntry, DirEntryError>;

#[derive(Default, Clone)]
pub struct ReadOpts {
    pub show_hidden: bool,
    pub show_ignored: bool,
    pub mode_index: usize,
    pub recursion: Recursion,
    pub target_dir: PathBuf,
    pub expansions: HashSet<PathBuf>,
}

#[derive(Copy, Clone, clap::ValueEnum, PartialEq, Eq, Debug)]
pub enum Mode {
    Mixed = 0,
    Files = 1,
    Dirs = 2,
}

pub const MODES: [Mode; 3] = [Mode::Mixed, Mode::Files, Mode::Dirs];

#[derive(Copy, Clone, PartialEq, Eq)]
pub enum Recursion {
    None = 0,
    All = 1,
}

impl Default for Recursion {
    fn default() -> Self {
        Self::None
    }
}

impl Recursion {
    pub fn next(&self) -> Self {
        match self {
            Self::None => Self::All,
            Self::All => Self::None,
        }
    }
}

pub fn stream_content(tx: AddItem, src: impl AsRef<Path>, read_opts: &ReadOpts) -> Result<()> {
    let src = src.as_ref();
    if read_opts.recursion == Recursion::None {
        for exp in &read_opts.expansions {
            stream_rel_content(tx.clone(), src, exp, read_opts);
        }
    }
    stream_rel_content(tx.clone(), src, src, read_opts);

    Ok(())
}

pub fn stream_rel_content(
    tx: AddItem,
    root: impl AsRef<Path>,
    src: impl AsRef<Path>,
    read_opts: &ReadOpts,
) {
    let root = root.as_ref().to_path_buf();

    /* @return true if we should early exit */
    let maybe_send = |tx: &AddItem, f: Item| {
        if let Item::FileEntry { info, .. } = &f {
            match MODES[read_opts.mode_index] {
                Mode::Mixed => (),
                Mode::Files => {
                    if !info.file_type.is_file() {
                        return false;
                    }
                }
                Mode::Dirs => {
                    if !info.file_type.is_dir() {
                        return false;
                    }
                }
            }
        }

        tx.send(f).is_err()
    };

    let max_depth = match read_opts.recursion {
        Recursion::None => Some(1),
        Recursion::All => None,
    };

    let ignore_files = !read_opts.show_ignored;
    let src = src.as_ref().to_path_buf();
    let mut walk = WalkBuilder::new(src);
    let walk = walk
        .follow_links(true)
        .hidden(!read_opts.show_hidden)
        .ignore(ignore_files)
        .git_exclude(ignore_files)
        .git_global(ignore_files)
        .git_ignore(ignore_files)
        .max_depth(max_depth);

    walk.build_parallel().run(|| {
        let tx = tx.clone();
        let root = root.clone();
        Box::new(move |f: DResult| {
            if let Some(item) = convert(&root, f) {
                if maybe_send(&tx, item) {
                    WalkState::Quit
                } else {
                    WalkState::Continue
                }
            } else {
                WalkState::Continue
            }
        })
    });
}
