use anyhow::{anyhow, Context};
use anyhow::{bail, Result};
use clap::Parser;
use ignore::{DirEntry, Error, WalkBuilder, WalkState};
use skim::prelude::*;
use std::ffi::OsString;
use std::fs::FileType;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::{fs, thread};
use tuikit::attr::{Attr, Color};

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    #[clap(default_value = ".")]
    start_path: OsString,

    #[clap(short, long)]
    query: Option<String>,

    #[clap(short, long)]
    recursive: bool,

    /// default: mixed (when non-recursive), files (when recursive)
    #[clap(short, long)]
    mode: Option<Mode>,
}

#[derive(Eq, PartialEq)]
struct FileName {
    name: OsString,
    file_type: FileType,
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

fn colour_whole(s: String, attr: impl Into<Attr>) -> AnsiString<'static> {
    let whole = (0, s.len() as u32);
    AnsiString::new_string(s, vec![(attr.into(), whole)])
}

#[derive(Default, Clone)]
struct ReadOpts {
    sort: bool,
    show_hidden: bool,
    show_ignored: bool,
    mode_index: usize,
    recursion_index: usize,
    target_dir: PathBuf,
}

#[derive(Copy, Clone, clap::ValueEnum, PartialEq, Eq)]
enum Mode {
    Mixed = 0,
    Files = 1,
    Dirs = 2,
}

const MODES: [Mode; 3] = [Mode::Mixed, Mode::Files, Mode::Dirs];

#[derive(Copy, Clone, PartialEq, Eq)]
enum Recursion {
    None = 0,
    Target = 1,
    All = 2,
}

const RECURSION: [Recursion; 3] = [Recursion::None, Recursion::Target, Recursion::All];

#[derive(Copy, Clone, PartialEq, Eq)]
enum Action {
    Up,
    Down,
    Home,
    CycleSort,
    CycleHidden,
    CycleIgnored,
    CycleMode,
    CycleRecursion,
    SetTarget,
    Open,
    Return,
}

fn main() -> Result<ExitCode> {
    let cli = Cli::parse();
    let mut here = fs::canonicalize(cli.start_path).context("start path")?;
    let mut options = SkimOptions::default();
    options.no_clear = true;
    if let Some(query) = cli.query {
        options.query = Some(query);
    }

    let mut read_opts = ReadOpts::default();
    read_opts.target_dir = here.clone();

    if let Some(mode) = cli.mode {
        read_opts.mode_index = mode as usize;
    } else if cli.recursive {
        read_opts.mode_index = Mode::Files as usize;
    } else {
        read_opts.mode_index = Mode::Mixed as usize;
    }

    if cli.recursive {
        read_opts.recursion_index = Recursion::All as usize;
    }

    let bindings = vec![
        (Key::Left, Action::Up),
        (Key::Ctrl('h'), Action::Up),
        (Key::Right, Action::Down),
        (Key::Ctrl('l'), Action::Down),
        (Key::Ctrl('d'), Action::Home),
        (Key::Ctrl('s'), Action::CycleSort),
        (Key::Ctrl('a'), Action::CycleHidden),
        (Key::Ctrl('y'), Action::CycleIgnored),
        (Key::Ctrl('f'), Action::CycleMode),
        (Key::Char('\\'), Action::CycleRecursion),
        (Key::Ctrl('t'), Action::SetTarget),
        (Key::Ctrl('g'), Action::Open),
    ];

    for (key, _) in &bindings {
        options.bind.push(format!("{}:abort", render_key(*key)));
    }

    loop {
        if here.as_os_str().as_encoded_bytes().len()
            < read_opts.target_dir.as_os_str().as_encoded_bytes().len()
        {
            read_opts.target_dir = here.clone();
        }

        let (tx, rx) = unbounded::<Arc<dyn SkimItem>>();
        options.prompt = format!("{} > ", here.to_string_lossy());
        let here_copy = here.clone();
        let read_opts_copy = read_opts.clone();
        let streamer = thread::spawn(move || stream_content(tx, here_copy, &read_opts_copy));

        let output = Skim::run_with(&options, Some(rx)).ok_or_else(|| anyhow!("skim said NONE"))?;

        streamer.join().expect("panic")?;

        options.query = Some(output.query);

        let item = output.selected_items.into_iter().next();

        let item = item.as_ref().map(|item| {
            (**item)
                .as_any()
                .downcast_ref::<FileName>()
                .expect("single type")
        });

        match bindings
            .iter()
            .find_map(|(key, action)| {
                if output.final_key == *key {
                    Some(*action)
                } else {
                    None
                }
            })
            .unwrap_or(Action::Return)
        {
            Action::Up => {
                here.pop();
            }
            Action::Down => {
                if let Some(item) = item {
                    if let Ok(cand) = ensure_directory(here.join(&item.name)) {
                        here = cand;
                    }
                }
            }
            Action::Home => {
                here = dirs::home_dir()
                    .ok_or_else(|| anyhow!("but you don't even have a home dir"))?;
            }
            Action::CycleSort => {
                read_opts.sort = !read_opts.sort;
            }
            Action::CycleHidden => {
                read_opts.show_hidden = !read_opts.show_hidden;
            }
            Action::CycleIgnored => {
                read_opts.show_ignored = !read_opts.show_ignored;
            }
            Action::CycleMode => {
                read_opts.mode_index = (read_opts.mode_index + 1) % MODES.len();
            }
            Action::CycleRecursion => {
                read_opts.recursion_index = (read_opts.recursion_index + 1) % RECURSION.len();
            }
            Action::SetTarget => {
                read_opts.target_dir = here.clone();
            }
            Action::Open => {
                if let Some(item) = item {
                    open::that_detached(here.join(&item.name))?;
                }
            }
            Action::Return => {
                return if output.is_abort {
                    Ok(ExitCode::FAILURE)
                } else if let Some(item) = item {
                    println!("{}", here.join(&item.name).to_string_lossy());
                    Ok(ExitCode::SUCCESS)
                } else {
                    Ok(ExitCode::FAILURE)
                }
            }
        }
    }
}

fn stream_content(
    tx: Sender<Arc<dyn SkimItem>>,
    src: impl AsRef<Path>,
    read_opts: &ReadOpts,
) -> Result<()> {
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
            .collect::<Result<Vec<_>>>()?;
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
    fn convert(root: impl AsRef<Path>, f: DirEntry) -> Result<Self> {
        let name = f.path().strip_prefix(root)?.as_os_str().to_owned();
        let file_type = f
            .file_type()
            .with_context(|| anyhow!("retrieving type of {:?}", &name))?;

        Ok(FileName { name, file_type })
    }
}

fn ensure_directory(p: impl AsRef<Path>) -> Result<PathBuf> {
    let canon = fs::canonicalize(p)?;
    if !fs::metadata(&canon)?.is_dir() {
        bail!("resolved path is not a directory");
    }

    Ok(canon)
}

fn render_key(key: Key) -> String {
    use Key::*;
    match key {
        Left => "left".to_string(),
        Right => "right".to_string(),

        Char(c) => format!("{c}"),
        Ctrl(c) => format!("ctrl-{c}"),
        other => unimplemented!("no rendering for {other:?}"),
    }
}
