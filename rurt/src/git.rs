use crate::cache::Cache;
use anyhow::Result;
use gix::revision::walk::Sorting;
use gix::Repository;
use std::cell::RefCell;
use std::path::{Path, PathBuf};

pub struct Git {
    root: PathBuf,
    repo: Repository,
    cache: RefCell<Cache<PathBuf, String>>,
}

impl Git {
    pub fn new(here: impl AsRef<Path>) -> Option<Self> {
        let repo = gix::discover(&here).ok()?;
        let root = repo.work_dir()?.to_path_buf();
        Some(Self {
            root,
            repo,
            cache: RefCell::new(Cache::new()),
        })
    }

    pub fn resolve(&self, path: impl AsRef<Path>) -> Option<String> {
        let path = path.as_ref();
        let repo = self.repo.clone();
        let root = self.root.clone();
        let path_2 = path.to_path_buf();
        self.cache
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
