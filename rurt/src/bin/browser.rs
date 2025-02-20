use anyhow::{anyhow, Context};
use anyhow::{bail, Result};
use clap::Parser;
use crossterm::event::{KeyCode, KeyModifiers};
use nucleo::Nucleo;
use rurt::dir_stack::DirStack;
use rurt::item::Item;
use rurt::ratui::run;
use rurt::store::Store;
use rurt::walk::{Mode, ReadOpts, Recursion, MODES, RECURSION};
use rurt::App;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;

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
    let here = fs::canonicalize(cli.start_path).context("start path")?;

    let mut app = App {
        dir_stack: DirStack::default(),
        read_opts: ReadOpts {
            target_dir: here.clone(),
            ..Default::default()
        },
        here,
    };

    if let Some(mode) = cli.mode {
        app.read_opts.mode_index = mode as usize;
    } else if cli.recursive {
        app.read_opts.mode_index = Mode::Files as usize;
    } else {
        app.read_opts.mode_index = Mode::Mixed as usize;
    }

    if cli.recursive {
        app.read_opts.recursion_index = Recursion::All as usize;
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

    let mut store = Store::new(Nucleo::<Item>::new(
        nucleo::Config::DEFAULT,
        Arc::new(|| {}),
        None,
        1,
    ));

    loop {
        // options.preview = Some(get_preview_command(&here));
        if app.here.as_os_str().as_encoded_bytes().len()
            < app
                .read_opts
                .target_dir
                .as_os_str()
                .as_encoded_bytes()
                .len()
        {
            app.read_opts.target_dir.clone_from(&app.here);
        }

        store.start_scan(&app)?;

        let (final_key, item) = run(
            &mut store.nucleo,
            format!("{}> ", app.here.to_string_lossy()),
        )?;

        // as we are just about to blow up the nucleo index uncondintionally
        let item = item.cloned();

        store.cancel_scan()?;

        let picked_action = bindings
            .iter()
            .find_map(|(modifier, key, action)| {
                if final_key.code == *key && final_key.modifiers == *modifier {
                    Some(*action)
                } else {
                    None
                }
            })
            .unwrap_or(Action::Ignore);

        match handle_action(picked_action, &mut app, item.as_ref())? {
            ActionResult::Ignored => (),
            ActionResult::Configured => (),
            ActionResult::Navigated => {
                app.read_opts.expansions.clear();
            }
            ActionResult::Exit(code) => return Ok(code),
        }
    }
}

enum ActionResult {
    Ignored,
    Navigated,
    Configured,
    Exit(ExitCode),
}

fn handle_action(axion: Action, app: &mut App, item: Option<&Item>) -> Result<ActionResult> {
    let here = &mut app.here;
    let read_opts = &mut app.read_opts;
    let dir_stack = &mut app.dir_stack;

    Ok(match axion {
        Action::Up => {
            dir_stack.push(here.clone());
            here.pop();
            ActionResult::Navigated
        }
        Action::Down => {
            if let Some(Item::FileEntry { name, .. }) = item {
                if let Ok(cand) = ensure_directory(here.join(name)) {
                    dir_stack.push(here.clone());
                    *here = cand;
                    return Ok(ActionResult::Navigated);
                }
            }
            ActionResult::Ignored
        }
        Action::Home => {
            dir_stack.push(here.clone());
            *here =
                dirs::home_dir().ok_or_else(|| anyhow!("but you don't even have a home dir"))?;
            ActionResult::Navigated
        }
        Action::CycleSort => {
            read_opts.sort = !read_opts.sort;
            ActionResult::Configured
        }
        Action::CycleHidden => {
            read_opts.show_hidden = !read_opts.show_hidden;
            ActionResult::Configured
        }
        Action::CycleIgnored => {
            read_opts.show_ignored = !read_opts.show_ignored;
            ActionResult::Configured
        }
        Action::CycleMode => {
            read_opts.mode_index = (read_opts.mode_index + 1) % MODES.len();
            ActionResult::Configured
        }
        Action::CycleRecursion => {
            read_opts.recursion_index = (read_opts.recursion_index + 1) % RECURSION.len();
            ActionResult::Configured
        }
        Action::TogglePreview => {
            /*
              options.preview = match options.preview {
                  None => Some(get_preview_command(&here)),
                  Some(_) => None,
              }
            */
            ActionResult::Configured
        }
        Action::SetTarget => {
            read_opts.target_dir.clone_from(&here);
            ActionResult::Configured
        }
        Action::Open => {
            if let Some(Item::FileEntry { name, .. }) = item {
                open::that_detached(here.join(name))?;
            }
            ActionResult::Ignored
        }
        Action::Expand => {
            if let Some(Item::FileEntry { name, .. }) = item {
                read_opts.expansions.push(here.join(name));
                ActionResult::Navigated
            } else {
                ActionResult::Ignored
            }
        }
        Action::DirBack => {
            if let Some(dir) = dir_stack.back(here.clone()) {
                *here = dir;
                ActionResult::Navigated
            } else {
                ActionResult::Ignored
            }
        }
        Action::DirForward => {
            if let Some(buf) = dir_stack.forward() {
                *here = buf;
                ActionResult::Navigated
            } else {
                ActionResult::Ignored
            }
        }
        Action::Abort => ActionResult::Exit(ExitCode::FAILURE),
        Action::Default => {
            if let Some(Item::FileEntry { name, .. }) = item {
                if let Ok(cand) = ensure_directory(here.join(name)) {
                    dir_stack.push(here.to_path_buf());
                    *here = cand;
                    ActionResult::Navigated
                } else {
                    println!("{}", here.join(name).to_string_lossy());
                    ActionResult::Exit(ExitCode::SUCCESS)
                }
            } else {
                ActionResult::Exit(ExitCode::FAILURE)
            }
        }
        Action::Ignore => ActionResult::Ignored,
    })
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
