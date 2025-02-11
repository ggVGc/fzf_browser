use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::Context;
use ignore::{DirEntry, Error, WalkBuilder, WalkState};
use skim::prelude::Sender;
use skim::SkimItem;

use crate::item::{convert, Item};

#[derive(Default, Clone)]
pub struct ReadOpts {
    pub sort: bool,
    pub show_hidden: bool,
    pub show_ignored: bool,
    pub mode_index: usize,
    pub recursion_index: usize,
    pub target_dir: PathBuf,
    pub expansions: Vec<PathBuf>,
}

#[derive(Copy, Clone, clap::ValueEnum, PartialEq, Eq)]
pub enum Mode {
    Mixed = 0,
    Files = 1,
    Dirs = 2,
}

pub const MODES: [Mode; 3] = [Mode::Mixed, Mode::Files, Mode::Dirs];

#[derive(Copy, Clone, PartialEq, Eq)]
pub enum Recursion {
    None = 0,
    Target = 1,
    All = 2,
}

pub const RECURSION: [Recursion; 3] = [Recursion::None, Recursion::Target, Recursion::All];

pub fn stream_content(tx: Sender<Arc<dyn SkimItem>>, src: impl AsRef<Path>, read_opts: &ReadOpts) {
    let src = src.as_ref();
    if RECURSION[read_opts.recursion_index] == Recursion::None {
        for exp in &read_opts.expansions {
            stream_rel_content(tx.clone(), src, exp, read_opts);
        }
    }
    stream_rel_content(tx.clone(), src, src, read_opts);
}

pub fn stream_rel_content(
    tx: Sender<Arc<dyn SkimItem>>,
    root: impl AsRef<Path>,
    src: impl AsRef<Path>,
    read_opts: &ReadOpts,
) {
    let mut src = src.as_ref().to_path_buf();
    let root = root.as_ref().to_path_buf();

    /* @return true if we should early exit */
    let maybe_send = |tx: &Sender<Arc<dyn SkimItem>>, f: Item| {
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

        // err: disconnected
        tx.send(Arc::new(f)).is_err()
    };

    let max_depth = match RECURSION[read_opts.recursion_index] {
        Recursion::None => Some(1),
        Recursion::Target => {
            src.clone_from(&read_opts.target_dir);
            None
        }
        Recursion::All => None,
    };

    let ignore_files = !read_opts.show_ignored;
    let mut walk = WalkBuilder::new(&src);
    let walk = walk
        .follow_links(true)
        .hidden(!read_opts.show_hidden)
        .ignore(ignore_files)
        .git_exclude(ignore_files)
        .git_global(ignore_files)
        .git_ignore(ignore_files)
        .max_depth(max_depth);

    if read_opts.sort {
        let mut files = walk
            .build()
            .filter_map(|item| convert(&root, item.context("dir walker")))
            .collect::<Vec<_>>();
        files.sort_unstable();
        for item in files {
            if maybe_send(&tx, item) {
                break;
            }
        }
    } else {
        walk.build_parallel().run(|| {
            let tx = tx.clone();
            let root = root.clone();
            Box::new(move |f: Result<DirEntry, Error>| {
                if let Some(item) = convert(&root, f.context("parallel walker")) {
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
}
