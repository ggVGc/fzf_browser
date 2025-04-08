use crate::cache::Cache;
use anyhow::Result;
use gix::bstr::{BString, ByteSlice};
use gix::diff::index::ChangeRef;
use gix::path::try_into_bstr;
use gix::progress::Discard;
use gix::revision::walk::Sorting;
use gix::status::index_worktree::Item as IndexItem;
use gix::status::Item as StatusItem;
use gix::Repository;
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub struct Git {
    root: PathBuf,
    repo: Repository,
    status: RefCell<Cache<PathBuf, HashMap<BString, Letter>>>,
    resolved: RefCell<Cache<PathBuf, String>>,
}

#[derive(Copy, Clone, Debug)]
pub enum Letter {
    SA,
    SD,
    SM,
    SR,
    SQ,
    UD,
    UM,
    UR,
    UQ,
}

impl Git {
    pub fn new(here: impl AsRef<Path>) -> Option<Self> {
        let repo = gix::discover(&here).ok()?;
        let root = repo.workdir()?.to_path_buf();
        let g = Self {
            root: root.clone(),
            repo: repo.clone(),
            status: RefCell::new(Cache::new()),
            resolved: RefCell::new(Cache::new()),
        };
        g.status
            .borrow_mut()
            .compute(root, move || status(&repo).ok());
        Some(g)
    }

    pub fn status(&self, abs: impl AsRef<Path>) -> Option<Letter> {
        let bstr = try_into_bstr(abs.as_ref().strip_prefix(&self.root).ok()?).ok()?;
        self.status
            .borrow_mut()
            .get(&self.root)?
            .get(&BString::from(bstr.as_bytes()))
            .cloned()
    }

    pub fn resolve(&self, path: impl AsRef<Path>) -> Option<String> {
        let path = path.as_ref();
        let repo = self.repo.clone();
        let root = self.root.clone();
        let path_2 = path.to_path_buf();
        self.resolved
            .borrow_mut()
            .compute(path.to_path_buf(), move || {
                find_file_commit(&repo, &root, path_2).ok().flatten()
            })
            .cloned()
    }
}

fn find_file_commit(
    repo: &Repository,
    root: impl AsRef<Path>,
    target: impl AsRef<Path>,
) -> Result<Option<String>> {
    let target = target.as_ref().strip_prefix(root)?;
    Ok(repo
        .rev_walk([repo.head()?.into_peeled_id()?])
        .sorting(Sorting::ByCommitTime(Default::default()))
        .all()?
        .find_map(|info| {
            let info = info.ok()?;
            let here = repo
                .rev_parse_single(format!("{}:{}", info.id, target.display()).as_str())
                .ok()?;
            if info.parent_ids.iter().any(|id| {
                repo.rev_parse_single(format!("{id}:{}", target.display()).as_str())
                    .ok()
                    != Some(here)
            }) {
                Some(info.object().ok()?.message().ok()?.summary().to_string())
            } else {
                None
            }
        }))
}

pub fn status(repo: &Repository) -> Result<HashMap<BString, Letter>> {
    let mut status = HashMap::with_capacity(8);
    for f in repo.status(Discard)?.into_iter([])? {
        let f = f?;
        let loc = f.location().to_owned();
        let letter = match f {
            // Two enums named Item inside each other? Everyone is fired.

            // "staged"
            StatusItem::TreeIndex(change) => match change {
                ChangeRef::Addition { .. } => Letter::SA,
                ChangeRef::Deletion { .. } => Letter::SD,
                ChangeRef::Modification { .. } => Letter::SM,
                ChangeRef::Rewrite { .. } => Letter::SR,
            },

            // "not staged"
            StatusItem::IndexWorktree(item) => match item {
                IndexItem::Modification { .. } => Letter::UM,
                IndexItem::Rewrite { .. } => Letter::UR,
                IndexItem::DirectoryContents { .. } => Letter::UQ,
            },
        };

        status.insert(loc, letter);
    }

    Ok(status)
}
