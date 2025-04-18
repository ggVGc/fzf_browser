use crate::colour::Colour;
use crate::git::Letter;
use crate::walk::DResult;
use anyhow::{anyhow, Context, Result};
use crossterm::style::ContentStyle;
use ignore::{DirEntry, Error as DError};
use lscolors::{Colorable, LsColors, Style as LsStyle};
use ratatui::prelude::Style as RStyle;
use ratatui::prelude::*;
use std::borrow::Cow;
use std::ffi::OsString;
use std::fs;
use std::fs::FileType;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(PartialEq, Eq, Clone, Debug)]
pub enum Item {
    FileEntry { name: OsString, info: Arc<ItemInfo> },
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

#[derive(Default)]
pub struct ItemView<'a> {
    pub primary: Vec<Span<'a>>,
    pub short: Vec<Span<'a>>,
    pub secondary: Option<Vec<Span<'a>>>,
    pub annotation: Vec<Span<'a>>,
    pub extra: Option<Vec<Span<'a>>>,
    pub directory: Option<String>,
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

    // rot: 0: fresh, 1: stale
    #[cfg(not(feature = "dirs_in_secondary"))]
    pub fn render(&self, context: &ViewContext) -> ItemView {
        match self {
            Item::WalkError { msg } => {
                return ItemView {
                    primary: vec![Span::styled(
                        format!("error walking: {msg}"),
                        context.styling.error,
                    )],
                    ..Default::default()
                };
            }
            Item::FileEntry { name, info, .. } => {
                return render_file_entry(name, info, context.styling, context.rot, context)
            }
        };
    }
}

pub struct ViewContext<'a> {
    pub git_status: Option<Letter>,
    pub git_info: Option<String>,
    pub rot: f32,
    pub styling: &'a Styling,
}

// rot: 0: fresh, 1: stale
#[cfg(not(feature = "dirs_in_secondary"))]
fn render_file_entry<'a>(
    name: &OsString,
    info: &Arc<ItemInfo>,
    styling: &Styling,
    rot: f32,
    context: &ViewContext,
) -> ItemView<'a> {
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

    let push_styled_path = |out: &mut Vec<Span<'a>>| {
        if let Some(style) = styling.item(info.as_ref()) {
            let style = LsStyle::to_crossterm_style(style);
            out.push(Span::styled(path.to_string(), style));
        } else {
            out.push(Span::raw(path.to_string()));
        }
    };

    let mut view = ItemView {
        primary: Vec::with_capacity(4),
        directory: dir.clone(),
        ..Default::default()
    };

    push_styled_path(&mut view.short);

    view.primary.push(Span::raw(" ["));
    if let Some(dir) = dir.clone() {
        for part in dir.split('/') {
            view.primary
                .push(Span::styled(part.to_string(), styling.dir));
            view.primary.push(Span::styled("|", styling.path_separator));
        }
    }

    push_styled_path(&mut view.primary);
    view.primary.push(Span::raw("]"));

    if let Some(link_dest) = &info.link_dest {
        let diff = info
            .path
            .parent()
            .and_then(|parent| pathdiff::diff_paths(link_dest, parent));
        let link_dest = match diff {
            Some(diff) if diff.components().count() > link_dest.components().count() => {
                link_dest.to_path_buf()
            }
            Some(diff) => diff,
            None => link_dest.to_path_buf(),
        };
        view.primary.push(Span::styled(" -> ", styling.symlink));
        view.primary
            .push(Span::raw(link_dest.display().to_string()));
    }
    for span in &mut view.primary {
        if let Some(colour) = span.style.fg {
            if let Ok(colour) = Colour::try_from(colour) {
                span.style.fg = Some(colour.desaturate(rot).into());
            }
        }
    }

    view.annotation = if let Some(git_status) = context.git_status {
        vec![Span::styled(format!("[{git_status:?}]"), styling.git_info)]
    } else {
        vec![Span::raw("    ")]
    };

    if let Some(git_info) = &context.git_info {
        view.extra = Some(vec![Span::styled(format!("{git_info}"), styling.git_info)])
    }

    view
}

#[cfg(feature = "dirs_in_secondary")]
pub fn render(
    &self,
    styling: &Styling,
    rot: f32,
    git_status: Option<Letter>,
    git_info: Option<&str>,
) -> ItemView {
    let (name, info) = match self {
        Item::WalkError { msg } => {
            return ItemView {
                primary: vec![Span::styled(format!("error walking: {msg}"), styling.error)],
                secondary: None,
                extra: None,
            };
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

    let mut cols = ItemView {
        primary: Vec::with_capacity(4),
        secondary: None,
        extra: None,
    };

    cols.secondary = if let Some(dir) = dir.clone() {
        let mut secondary = Vec::with_capacity(4);
        secondary.push(Span::raw("["));
        secondary.push(Span::raw(dir.to_string()));
        secondary.push(Span::raw("]"));
        Some(secondary)
    } else {
        Some(vec![Span::raw(".")])
    };

    if info.file_type.is_dir() {
        if let Some(dir) = dir {
            let mut indentation = 1;
            for ch in dir.chars() {
                if ch == '/' {
                    indentation = indentation + 1;
                }
            }
            let indents = String::from_utf8(vec![b' '; (indentation * 2) as usize]).unwrap();
            cols.primary.push(Span::raw(indents));
        }
    }

    if let Some(git_status) = git_status {
        cols.primary
            .push(Span::styled(format!("{git_status:?} "), styling.git_info));
    } else {
        cols.primary.push(Span::raw("   "));
    }

    if let Some(style) = styling.item(info.as_ref()) {
        let style = LsStyle::to_crossterm_style(style);
        cols.primary.push(Span::styled(path.to_string(), style));
    } else {
        cols.primary.push(Span::raw(path.to_string()));
    }

    if let Some(link_dest) = &info.link_dest {
        let link_dest =
            pathdiff::diff_paths(&info.path, link_dest).unwrap_or_else(|| link_dest.to_path_buf());
        cols.primary.push(Span::styled(" -> ", styling.symlink));
        cols.primary
            .push(Span::raw(link_dest.display().to_string()));
    }
    for span in &mut cols.primary {
        if let Some(colour) = span.style.fg {
            if let Ok(colour) = Colour::try_from(colour) {
                span.style.fg = Some(colour.desaturate(rot).into());
            }
        }
    }

    if let Some(git_info) = git_info {
        cols.extra = Some(vec![Span::styled(
            format!("  {git_info}"),
            styling.git_info,
        )]);
    }

    cols
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

pub fn convert(root: impl AsRef<Path>, f: DResult) -> Option<Item> {
    convert_resolution(root, f).unwrap_or_else(|e| {
        Some(Item::WalkError {
            msg: cerialise_error(e),
        })
    })
}

fn cerialise_error(e: anyhow::Error) -> String {
    let mut msg = String::new();
    for cause in e.chain() {
        msg.push_str(&format!("{:?} -- ", cause));
    }
    msg
}

fn convert_resolution(root: impl AsRef<Path>, f: DResult) -> Result<Option<Item>> {
    match f {
        Ok(f) => convert_resolution_entry(root, f),
        // it's assumed that these are almost exclusively broken symlinks
        Err(DError::WithPath { path, .. }) => convert_resolution_path(root, path),
        Err(e) => Err(e.into()),
    }
}

fn convert_resolution_entry(root: impl AsRef<Path>, f: DirEntry) -> Result<Option<Item>> {
    let path = f.path().to_path_buf();

    let name = path.strip_prefix(root)?.as_os_str().to_owned();
    let file_type = f
        .file_type()
        .with_context(|| anyhow!("retrieving type of {:?}", &name))?;

    // Skip root directory
    if f.depth() == 0 {
        return Ok(None);
    }

    let link_dest = if f.path_is_symlink() {
        fs::canonicalize(&path).ok()
    } else {
        None
    };

    Ok(Some(Item::FileEntry {
        name,
        info: Arc::new(ItemInfo {
            path,
            filename: f.file_name().to_os_string(),
            metadata: f.metadata().ok(),
            file_type,
            link_dest,
        }),
    }))
}

fn convert_resolution_path(root: impl AsRef<Path>, path: PathBuf) -> Result<Option<Item>> {
    let name = path.strip_prefix(root)?.as_os_str().to_owned();
    let filename = path
        .file_name()
        .ok_or_else(|| anyhow!("unexpected non-regular path in directory walk: {path:?}"))?
        .to_os_string();

    if name.is_empty() {
        return Ok(None);
    }

    let metadata = path
        .symlink_metadata()
        .context("metadata on an entry that's already in error")?;

    Ok(Some(Item::FileEntry {
        name,
        info: Arc::new(ItemInfo {
            filename,
            file_type: metadata.file_type(),
            link_dest: fs::read_link(&path).ok(),
            metadata: Some(metadata),
            path,
        }),
    }))
}

pub struct Styling {
    ls_colors: LsColors,
    pub path_separator: Style,
    pub dir: ContentStyle,
    pub error: Style,
    pub symlink: Style,
    pub git_info: Style,
}

impl Styling {
    pub fn new(ls_colors: &LsColors) -> Self {
        let dir_style = ls_colors
            .style_for_indicator(lscolors::Indicator::Directory)
            .unwrap();

        Self {
            ls_colors: ls_colors.clone(),
            dir: lscolors::Style::to_crossterm_style(dir_style),
            path_separator: RStyle::new().fg(Color::Indexed(139)),
            symlink: RStyle::new().light_magenta(),
            error: RStyle::default().light_red(),
            git_info: RStyle::default().fg(Color::DarkGray),
        }
    }

    pub fn item(&self, item: &ItemInfo) -> Option<&LsStyle> {
        self.ls_colors.style_for(item)
    }
}
