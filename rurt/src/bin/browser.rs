use anyhow::{anyhow, Context};
use anyhow::{bail, Result};
use clap::Parser;
use crossterm::event::{KeyCode, KeyModifiers};
use nucleo::Nucleo;
use rurt::dir_stack::DirStack;
use rurt::item::Item;
use rurt::ratui::run;
use rurt::walk::{stream_content, Mode, ReadOpts, Recursion, MODES, RECURSION};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;
use std::{fs, thread};

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
    Default,
    Ignore,
    Up,
    Down,
    Home,
    CycleSort,
    CycleHidden,
    CycleIgnored,
    CycleMode,
    CycleRecursion,
    TogglePreview,
    SetTarget,
    Expand,
    Open,
    DirBack,
    DirForward,
    Abort,
}

fn main() -> Result<ExitCode> {
    env_logger::init();
    let cli = Cli::parse();
    let mut here = fs::canonicalize(cli.start_path).context("start path")?;
    let mut dir_stack = DirStack::<PathBuf>::default();

    let mut read_opts = ReadOpts {
        target_dir: here.clone(),
        ..Default::default()
    };

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

    let bindings = [
        (KeyModifiers::NONE, KeyCode::Enter, Action::Default),
        (KeyModifiers::NONE, KeyCode::Esc, Action::Abort),
        (KeyModifiers::CONTROL, KeyCode::Char('c'), Action::Abort),
        (KeyModifiers::NONE, KeyCode::Left, Action::Up),
        (KeyModifiers::CONTROL, KeyCode::Char('h'), Action::Up),
        (KeyModifiers::NONE, KeyCode::Right, Action::Down),
        (KeyModifiers::CONTROL, KeyCode::Char('l'), Action::Down),
        (KeyModifiers::CONTROL, KeyCode::Char('d'), Action::Home),
        (KeyModifiers::CONTROL, KeyCode::Char('s'), Action::CycleSort),
        (
            KeyModifiers::CONTROL,
            KeyCode::Char('a'),
            Action::CycleHidden,
        ),
        (
            KeyModifiers::CONTROL,
            KeyCode::Char('y'),
            Action::CycleIgnored,
        ),
        (KeyModifiers::CONTROL, KeyCode::Char('f'), Action::CycleMode),
        (KeyModifiers::CONTROL, KeyCode::Char('e'), Action::Expand),
        (
            KeyModifiers::NONE,
            KeyCode::Char('\\'),
            Action::CycleRecursion,
        ),
        (
            KeyModifiers::CONTROL,
            KeyCode::Char('r'),
            Action::CycleRecursion,
        ),
        (KeyModifiers::CONTROL, KeyCode::Char('t'), Action::SetTarget),
        (KeyModifiers::CONTROL, KeyCode::Char('g'), Action::Open),
        (KeyModifiers::CONTROL, KeyCode::Char('o'), Action::DirBack),
        (
            KeyModifiers::CONTROL,
            KeyCode::Char('u'),
            Action::DirForward,
        ),
    ];

    loop {
        // options.preview = Some(get_preview_command(&here));
        if here.as_os_str().as_encoded_bytes().len()
            < read_opts.target_dir.as_os_str().as_encoded_bytes().len()
        {
            read_opts.target_dir.clone_from(&here);
        }

        let config = nucleo::Config::DEFAULT;
        let mut nucleo = Nucleo::<Item>::new(config, Arc::new(|| {}), None, 1);
        let tx = nucleo.injector();
        let here_copy = here.clone();
        let read_opts_copy = read_opts.clone();
        let streamer = thread::spawn(move || stream_content(tx, here_copy, &read_opts_copy));

        let (final_key, item) = run(&mut nucleo)?;

        streamer.join().expect("panic");

        let navigated = |read_opts: &mut ReadOpts| {
            read_opts.expansions.clear();
        };

        match bindings
            .iter()
            .find_map(|(modifier, key, action)| {
                if final_key.code == *key && final_key.modifiers == *modifier {
                    Some(*action)
                } else {
                    None
                }
            })
            .unwrap_or(Action::Ignore)
        {
            Action::Up => {
                dir_stack.push(here.clone());
                here.pop();
                navigated(&mut read_opts);
            }
            Action::Down => {
                if let Some(Item::FileEntry { name, .. }) = item {
                    if let Ok(cand) = ensure_directory(here.join(name)) {
                        dir_stack.push(here.clone());
                        here = cand;
                        navigated(&mut read_opts);
                    }
                }
            }
            Action::Home => {
                dir_stack.push(here.clone());
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
            Action::TogglePreview => {
              /*
                options.preview = match options.preview {
                    None => Some(get_preview_command(&here)),
                    Some(_) => None,
                }
              */
            }
            Action::SetTarget => {
                read_opts.target_dir.clone_from(&here);
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
            Action::DirBack => {
                if let Some(dir) = dir_stack.back(here.clone()) {
                    here = dir;
                    navigated(&mut read_opts);
                }
            }
            Action::DirForward => {
                if let Some(buf) = dir_stack.forward() {
                    here = buf;
                    navigated(&mut read_opts);
                }
            }
            Action::Abort => return Ok(ExitCode::FAILURE),
            Action::Default => {
                if let Some(Item::FileEntry { name, .. }) = item {
                    if let Ok(cand) = ensure_directory(here.join(name)) {
                        dir_stack.push(here);
                        here = cand;
                        navigated(&mut read_opts);
                        // options.query = None;
                    } else {
                        println!("{}", here.join(name).to_string_lossy());
                        return Ok(ExitCode::SUCCESS);
                    }
                } else {
                    return Ok(ExitCode::FAILURE);
                }
            }
            Action::Ignore => (),
        }
    }
}

fn get_preview_command(current_dir: &Path) -> String {
    format!(
        "fzf-browser-preview.sh {}/{}",
        current_dir.to_string_lossy(),
        "{}"
    )
}

fn ensure_directory(p: impl AsRef<Path>) -> Result<PathBuf> {
    let canon = fs::canonicalize(p)?;
    if !fs::metadata(&canon)?.is_dir() {
        bail!("resolved path is not a directory");
    }

    Ok(canon)
}

#[cfg(never)]
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
