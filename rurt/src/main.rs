mod item;
mod walk;

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::{fs, thread};

use anyhow::{anyhow, Context};
use anyhow::{bail, Result};
use clap::Parser;
use skim::prelude::*;

use crate::item::Item;
use crate::walk::{stream_content, Mode, ReadOpts, Recursion, MODES, RECURSION};

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
    Expand,
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
        (Key::Ctrl('e'), Action::Expand),
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

        streamer.join().expect("panic");

        options.query = Some(output.query);

        let item = output.selected_items.into_iter().next();

        let item = item.as_ref().map(|item| {
            (**item)
                .as_any()
                .downcast_ref::<Item>()
                .expect("single type")
        });

        let navigated = |read_opts: &mut ReadOpts| {
            read_opts.expansions.clear();
        };

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
                navigated(&mut read_opts);
            }
            Action::Down => {
                if let Some(Item::FileEntry { name, .. }) = item {
                    if let Ok(cand) = ensure_directory(here.join(name)) {
                        here = cand;
                        navigated(&mut read_opts);
                    }
                }
            }
            Action::Home => {
                here = dirs::home_dir()
                    .ok_or_else(|| anyhow!("but you don't even have a home dir"))?;
                navigated(&mut read_opts);
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
                if let Some(Item::FileEntry { name, .. }) = item {
                    open::that_detached(here.join(name))?;
                }
            }
            Action::Expand => {
                if let Some(Item::FileEntry { name, .. }) = item {
                    read_opts.expansions.push(here.join(name));
                }
            }
            Action::Return => {
                return if output.is_abort {
                    Ok(ExitCode::FAILURE)
                } else if let Some(Item::FileEntry { name, .. }) = item {
                    println!("{}", here.join(name).to_string_lossy());
                    Ok(ExitCode::SUCCESS)
                } else {
                    Ok(ExitCode::FAILURE)
                }
            }
        }
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
