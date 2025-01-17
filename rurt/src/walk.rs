use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::Context;
use ignore::{DirEntry, Error, WalkBuilder, WalkState};
use skim::prelude::Sender;
use skim::SkimItem;

use crate::item::FileName;

#[derive(Default, Clone)]
pub struct ReadOpts {
    pub sort: bool,
    pub show_hidden: bool,
    pub show_ignored: bool,
    pub mode_index: usize,
    pub recursion_index: usize,
    pub target_dir: PathBuf,
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

pub fn stream_content(
    tx: Sender<Arc<dyn SkimItem>>,
    src: impl AsRef<Path>,
    read_opts: &ReadOpts,
) -> anyhow::Result<()> {
    let mut src = src.as_ref().to_path_buf();

    /* @return true if we should early exit */
    let maybe_send = |tx: &Sender<Arc<dyn SkimItem>>, f: FileName| {
        match MODES[read_opts.mode_index] {
            Mode::Mixed => (),
            Mode::Files => {
                if !f.file_type.is_file() {
                    return false;
                }
            }
            Mode::Dirs => {
                if !f.file_type.is_dir() {
                    return false;
                }
            }
        }

        // err: disconnected
        tx.send(Arc::new(f)).is_err()
    };

    let max_depth = match RECURSION[read_opts.recursion_index] {
        Recursion::None => Some(1),
        Recursion::Target => {
            src = read_opts.target_dir.clone();
            None
        }
        Recursion::All => None,
    };

    let ignore_files = !read_opts.show_ignored;
    let mut walk = WalkBuilder::new(&src);
    let walk = walk
        .hidden(!read_opts.show_hidden)
        .ignore(ignore_files)
        .git_exclude(ignore_files)
        .git_global(ignore_files)
        .git_ignore(ignore_files)
        .max_depth(max_depth);

    if read_opts.sort {
        let mut files = walk
            .build()
            .into_iter()
            .map(|f| FileName::convert(&src, f?))
            .collect::<anyhow::Result<Vec<_>>>()?;
        files.sort_unstable();
        for f in files {
            if maybe_send(&tx, f) {
                break;
            }
        }
    } else {
        walk.build_parallel().run(|| {
            let tx = tx.clone();
            let src = src.clone();
            Box::new(move |f: std::result::Result<DirEntry, Error>| {
                let f = match f
                    .context("dir walker")
                    .and_then(|f| FileName::convert(&src, f))
                {
                    Ok(f) => f,
                    Err(_) => {
                        // TODO: ... FileItem can be an Error?
                        return WalkState::Continue;
                    }
                };

                if maybe_send(&tx, f) {
                    WalkState::Quit
                } else {
                    WalkState::Continue
                }
            })
        });
    }
    Ok(())
}
