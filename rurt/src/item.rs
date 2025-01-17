use std::borrow::Cow;
use std::ffi::OsString;
use std::fs::FileType;
use std::path::Path;

use anyhow::{anyhow, Context, Result};
use ignore::DirEntry;
use skim::{AnsiString, DisplayContext, SkimItem};
use tuikit::attr::{Attr, Color};

#[derive(PartialEq, Eq)]
pub enum Item {
    FileEntry { name: OsString, file_type: FileType },
    WalkError { msg: String },
}

impl SkimItem for Item {
    fn text(&self) -> Cow<str> {
        match self {
            Item::FileEntry { name, .. } => name.to_string_lossy(),
            Item::WalkError { msg } => msg.into(),
        }
    }

    fn display<'a>(&'a self, _context: DisplayContext<'a>) -> AnsiString<'a> {
        let (name, file_type) = match self {
            Item::WalkError { msg } => {
                return colour_whole(format!("error walking: {msg}"), Color::RED)
            }
            Item::FileEntry { name, file_type } => (name, file_type),
        };
        let s = name.to_string_lossy().to_string();
        if file_type.is_dir() {
            colour_whole(s, Color::LIGHT_BLUE)
        } else if file_type.is_symlink() {
            colour_whole(s, Color::LIGHT_CYAN)
        } else if file_type.is_file() {
            s.into()
        } else {
            colour_whole(s, Color::LIGHT_RED)
        }
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
                    name: an,
                    file_type: at,
                },
                Item::FileEntry {
                    name: bn,
                    file_type: bt,
                },
            ) => {
                let a = at.is_dir();
                let b = bt.is_dir();
                if a != b {
                    return a.cmp(&b);
                }
                an.cmp(bn)
            }
            (Item::WalkError { msg: a }, Item::WalkError { msg: b }) => a.cmp(b),
            (Item::FileEntry { .. }, Item::WalkError { .. }) => std::cmp::Ordering::Less,
            (Item::WalkError { .. }, Item::FileEntry { .. }) => std::cmp::Ordering::Greater,
        }
    }
}

pub fn convert(root: impl AsRef<Path>, f: Result<DirEntry>) -> Item {
    convert_resolution(root, f).unwrap_or_else(|e| Item::WalkError {
        msg: cerialise_error(e),
    })
}

fn cerialise_error(e: anyhow::Error) -> String {
    let mut msg = String::new();
    for cause in e.chain() {
        msg.push_str(&format!("{} -- ", cause));
    }
    msg
}

fn convert_resolution(root: impl AsRef<Path>, f: Result<DirEntry>) -> Result<Item> {
    let f = f?;
    let name = f.path().strip_prefix(root)?.as_os_str().to_owned();
    let file_type = f
        .file_type()
        .with_context(|| anyhow!("retrieving type of {:?}", &name))?;

    Ok(Item::FileEntry { name, file_type })
}

fn colour_whole(s: String, attr: impl Into<Attr>) -> AnsiString<'static> {
    let whole = (0, s.len() as u32);
    AnsiString::new_string(s, vec![(attr.into(), whole)])
}
