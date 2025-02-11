use std::ffi::OsString;
use std::fs::FileType;
use std::path::Path;
use std::{borrow::Cow, sync::Mutex};

use anyhow::{anyhow, Context, Result};
use ignore::DirEntry;
use lscolors::{Colorable, LsColors, Style};
use once_cell::sync::Lazy;
use skim::{AnsiString, DisplayContext, SkimItem};
use tuikit::attr::{Attr, Color};

static LS_COLORS: Lazy<Mutex<LsColors>> = Lazy::new(|| {
    let colors = LsColors::from_env().unwrap_or_default();
    Mutex::new(colors)
});

// #[derive(PartialEq, Eq)]
pub enum Item {
    FileEntry {
        name: OsString,
        info: ItemInfo,
        file_type: FileType,
    },
    WalkError {
        msg: String,
    },
}

pub struct ItemInfo {
    entry: DirEntry,
}

impl Colorable for ItemInfo {
    fn path(&self) -> std::path::PathBuf {
        self.entry.path().to_path_buf()
    }

    fn file_name(&self) -> OsString {
        self.entry.file_name().to_os_string()
    }

    fn file_type(&self) -> Option<FileType> {
        self.entry.file_type()
    }

    fn metadata(&self) -> Option<std::fs::Metadata> {
        self.entry.metadata().ok()
    }
}

impl SkimItem for Item {
    fn text(&self) -> Cow<str> {
        match self {
            Item::FileEntry { name, .. } => name.to_string_lossy(),
            Item::WalkError { msg } => msg.into(),
        }
    }

    fn display<'a>(&'a self, _context: DisplayContext<'a>) -> AnsiString<'a> {
        let (name, info) = match self {
            Item::WalkError { msg } => {
                return colour_whole(format!("error walking: {msg}"), Color::RED)
            }
            Item::FileEntry { name, info, .. } => (name, info),
        };

        let name = name.to_string_lossy();
        let lscolors = LS_COLORS.lock().unwrap();
        if let Some(style) = lscolors.style_for(info) {
            let style = Style::to_ansi_term_style(style);
            AnsiString::parse(style.paint(name).to_string().as_str())
        } else {
            AnsiString::parse(name.to_string().as_str())
        }
    }
}

impl PartialEq for Item {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::FileEntry { name: l_name, .. }, Self::FileEntry { name: r_name, .. }) => {
                l_name == r_name
            }
            (Self::WalkError { msg: l_msg }, Self::WalkError { msg: r_msg }) => l_msg == r_msg,
            _ => false,
        }
    }
}

impl Eq for Item {}

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
                    ..
                },
                Item::FileEntry {
                    name: bn,
                    file_type: bt,
                    ..
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
        Ok(Some(Item::FileEntry {
            name,
            file_type,
            info: ItemInfo { entry: f },
        }))
    } else {
        Ok(None)
    }
}

fn colour_whole(s: String, attr: impl Into<Attr>) -> AnsiString<'static> {
    let whole = (0, s.len() as u32);
    AnsiString::new_string(s, vec![(attr.into(), whole)])
}
