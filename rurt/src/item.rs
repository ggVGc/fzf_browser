use anyhow::{anyhow, Context, Result};
use ignore::DirEntry;
use lscolors::{Colorable, LsColors, Style as LsStyle};
use ratatui::prelude::Style as RStyle;
use ratatui::prelude::*;
use std::borrow::Cow;
use std::ffi::OsString;
use std::fs;
use std::fs::FileType;
use std::path::{Path, PathBuf};

#[derive(PartialEq, Eq, Clone, Debug)]
pub enum Item {
    FileEntry { name: OsString, info: Box<ItemInfo> },
    WalkError { msg: String },
}

#[derive(Clone, Debug)]
pub struct ItemInfo {
    pub file_type: FileType,
    path: PathBuf,
    filename: OsString,
    metadata: Option<fs::Metadata>,
    pub link_dest: Option<PathBuf>,
}

impl PartialEq for ItemInfo {
    fn eq(&self, other: &Self) -> bool {
        self.path.eq(&other.path)
    }
}

impl Eq for ItemInfo {}

impl Colorable for ItemInfo {
    fn path(&self) -> std::path::PathBuf {
        self.path.to_path_buf()
    }

    fn file_name(&self) -> OsString {
        self.filename.to_os_string()
    }

    fn file_type(&self) -> Option<FileType> {
        Some(self.file_type)
    }

    fn metadata(&self) -> Option<std::fs::Metadata> {
        self.metadata.clone()
    }
}

impl Item {
    pub fn text(&self) -> Cow<str> {
        match self {
            Item::FileEntry { name, .. } => name.to_string_lossy(),
            Item::WalkError { msg } => msg.into(),
        }
    }

    pub fn path(&self) -> Option<&Path> {
        match self {
            Item::FileEntry { info, .. } => Some(&info.path),
            Item::WalkError { .. } => None,
        }
    }

    pub fn as_span(&self, ls_colors: &LsColors) -> Vec<Span> {
        let (name, info) = match self {
            Item::WalkError { msg } => {
                return vec![Span::styled(
                    format!("error walking: {msg}"),
                    RStyle::default().light_red(),
                )];
            }
            Item::FileEntry { name, info, .. } => (name, info),
        };

        let full = name.display().to_string();
        let (dir, path) = match full.rfind('/') {
            Some(pos) => {
                let (dir, name) = full.split_at(pos + 1);
                let mut dir = dir.to_string();
                let _trailing_slash = dir.pop();
                (Some(dir), name.to_string())
            }
            None => (None, full),
        };

        let mut spans = Vec::with_capacity(4);
        if let Some(dir) = dir {
            spans.push(Span::styled(
                dir.to_string(),
                RStyle::new().fg(Color::LightBlue),
            ));
            spans.push(Span::styled("/", RStyle::new().fg(Color::LightYellow)));
        }

        if let Some(style) = ls_colors.style_for(info.as_ref()) {
            let style = LsStyle::to_crossterm_style(style);
            spans.push(Span::styled(path.to_string(), style));
        } else {
            spans.push(Span::raw(path.to_string()));
        }

        if let Some(link_dest) = &info.link_dest {
            let link_dest = pathdiff::diff_paths(&info.path, link_dest)
                .unwrap_or_else(|| link_dest.to_path_buf());
            spans.push(Span::styled(" -> ", RStyle::new().fg(Color::LightMagenta)));
            spans.push(Span::raw(link_dest.display().to_string()));
        }
        spans
    }
}

impl PartialOrd for Item {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Item {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match (self, other) {
            (
                Item::FileEntry {
                    name: an, info: at, ..
                },
                Item::FileEntry {
                    name: bn, info: bt, ..
                },
            ) => {
                let a = at.file_type.is_dir();
                let b = bt.file_type.is_dir();
                if a != b {
                    b.cmp(&a)
                } else {
                    an.cmp(bn)
                }
            }
            (Item::WalkError { msg: a }, Item::WalkError { msg: b }) => a.cmp(b),
            (Item::FileEntry { .. }, Item::WalkError { .. }) => std::cmp::Ordering::Less,
            (Item::WalkError { .. }, Item::FileEntry { .. }) => std::cmp::Ordering::Greater,
        }
    }
}

pub fn convert(root: impl AsRef<Path>, f: Result<DirEntry>) -> Option<Item> {
    convert_resolution(root, f).unwrap_or_else(|e| {
        Some(Item::WalkError {
            msg: cerialise_error(e),
        })
    })
}

fn cerialise_error(e: anyhow::Error) -> String {
    let mut msg = String::new();
    for cause in e.chain() {
        msg.push_str(&format!("{} -- ", cause));
    }
    msg
}

fn convert_resolution(root: impl AsRef<Path>, f: Result<DirEntry>) -> Result<Option<Item>> {
    let f = f?;
    let name = f.path().strip_prefix(root)?.as_os_str().to_owned();
    let file_type = f
        .file_type()
        .with_context(|| anyhow!("retrieving type of {:?}", &name))?;

    // Skip root directory
    if f.depth() != 0 {
        let link_dest = if f.path_is_symlink() {
            fs::canonicalize(f.path()).ok()
        } else {
            None
        };

        Ok(Some(Item::FileEntry {
            name,
            info: Box::new(ItemInfo {
                path: f.path().to_path_buf(),
                filename: f.file_name().to_os_string(),
                metadata: f.metadata().ok(),
                file_type,
                link_dest,
            }),
        }))
    } else {
        Ok(None)
    }
}
