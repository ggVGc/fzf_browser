use std::borrow::Cow;
use std::ffi::OsString;
use std::fs::FileType;
use std::path::Path;

use anyhow::{anyhow, Context};
use ignore::DirEntry;
use skim::{AnsiString, DisplayContext, SkimItem};
use tuikit::attr::{Attr, Color};

#[derive(Eq, PartialEq)]
pub struct FileName {
    pub name: OsString,
    pub file_type: FileType,
}

impl SkimItem for FileName {
    fn text(&self) -> Cow<str> {
        self.name.to_string_lossy()
    }

    fn display<'a>(&'a self, _context: DisplayContext<'a>) -> AnsiString<'a> {
        let s = self.name.to_string_lossy().to_string();
        if self.file_type.is_dir() {
            colour_whole(s, Color::LIGHT_BLUE)
        } else if self.file_type.is_symlink() {
            colour_whole(s, Color::LIGHT_CYAN)
        } else if self.file_type.is_file() {
            s.into()
        } else {
            colour_whole(s, Color::LIGHT_RED)
        }
    }
}

impl PartialOrd for FileName {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for FileName {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        let a = self.file_type.is_dir();
        let b = other.file_type.is_dir();
        if a != b {
            return a.cmp(&b);
        }
        self.name.cmp(&other.name)
    }
}

impl FileName {
    pub fn convert(root: impl AsRef<Path>, f: DirEntry) -> anyhow::Result<Self> {
        let name = f.path().strip_prefix(root)?.as_os_str().to_owned();
        let file_type = f
            .file_type()
            .with_context(|| anyhow!("retrieving type of {:?}", &name))?;

        Ok(FileName { name, file_type })
    }
}

fn colour_whole(s: String, attr: impl Into<Attr>) -> AnsiString<'static> {
    let whole = (0, s.len() as u32);
    AnsiString::new_string(s, vec![(attr.into(), whole)])
}
