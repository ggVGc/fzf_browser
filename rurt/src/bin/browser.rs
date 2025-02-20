use anyhow::Context;
use anyhow::Result;
use clap::Parser;
use crossterm::event::{KeyCode, KeyModifiers};
use nucleo::Nucleo;
use rurt::action::Action;
use rurt::dir_stack::DirStack;
use rurt::item::Item;
use rurt::ratui::run;
use rurt::store::Store;
use rurt::walk::{Mode, ReadOpts, Recursion};
use rurt::App;
use std::ffi::OsString;
use std::fs;
use std::path::Path;
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

fn main() -> Result<ExitCode> {
    env_logger::init();
    let cli = Cli::parse();
    let here = fs::canonicalize(cli.start_path).context("start path")?;

    let bindings = vec![
        (KeyModifiers::NONE, KeyCode::Enter, Action::Default),
        (KeyModifiers::NONE, KeyCode::Esc, Action::Abort),
        (KeyModifiers::CONTROL, KeyCode::Char('c'), Action::Abort),
        (KeyModifiers::NONE, KeyCode::Left, Action::Up),
        (KeyModifiers::CONTROL, KeyCode::Char('h'), Action::Up),
        (KeyModifiers::NONE, KeyCode::Right, Action::Down),
        (KeyModifiers::CONTROL, KeyCode::Char('l'), Action::Down),
        (KeyModifiers::NONE, KeyCode::Up, Action::MoveCursor(-1)),
        (KeyModifiers::NONE, KeyCode::Down, Action::MoveCursor(1)),
        (KeyModifiers::CONTROL, KeyCode::Char('d'), Action::Home),
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
        (KeyModifiers::CONTROL, KeyCode::Char('c'), Action::Abort),
        (KeyModifiers::NONE, KeyCode::Esc, Action::Abort),
        (KeyModifiers::CONTROL, KeyCode::Char('o'), Action::DirBack),
        (
            KeyModifiers::CONTROL,
            KeyCode::Char('u'),
            Action::DirForward,
        ),
    ];

    let mut app = App {
        dir_stack: DirStack::default(),
        read_opts: ReadOpts {
            target_dir: here.clone(),
            ..Default::default()
        },
        bindings,
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

    let mut store = Store::new(Nucleo::<Item>::new(
        nucleo::Config::DEFAULT,
        Arc::new(|| {}),
        None,
        1,
    ));

    let (msg, code) = run(&mut store, &mut app)?;
    if let Some(msg) = msg {
        println!("{}", msg);
    }
    Ok(code)
}

fn get_preview_command(current_dir: &Path) -> String {
    format!(
        "fzf-browser-preview.sh {}/{}",
        current_dir.to_string_lossy(),
        "{}"
    )
}
