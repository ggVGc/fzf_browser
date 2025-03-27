use anyhow::Context;
use anyhow::Result;
use clap::{Parser, ValueEnum};
use crossterm::event::{KeyCode, KeyModifiers};
use log::LevelFilter;
use nucleo::Nucleo;
use rurt::action::Action;
use rurt::dir_stack::DirStack;
use rurt::draw::RIGHT_PANE_HIDDEN;
use rurt::draw::{ViewOpts, PREVIEW_MODE, RIGHT_PANE};
use rurt::item::Item;
use rurt::ratui;
use rurt::store::Store;
use rurt::tui_log::LogWidgetState;
use rurt::tui_log::TuiLogger;
use rurt::walk::{Mode, ReadOpts, Recursion};
use rurt::App;
use rurt::ResultOpts;
use shell_quote::Quote;
use std::ffi::OsString;
use std::fs;
use std::fs::File;
use std::io::Write;
use std::process::ExitCode;
use std::sync::Arc;
use std::sync::Mutex;

#[derive(ValueEnum, Copy, Clone)]
#[clap(rename_all = "kebab_case")]
enum QuoteFor {
    Bash,
    Fish,
}

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    #[clap(default_value = ".")]
    start_path: OsString,

    #[clap(short, long)]
    output_path: Option<OsString>,

    #[clap(short, long, action)]
    #[clap(default_value = "true")]
    preview: Option<bool>,

    #[clap(short, long)]
    query: Option<String>,

    #[clap(short, long)]
    recursive: bool,

    /// default: mixed (when non-recursive), files (when recursive)
    #[clap(short, long)]
    mode: Option<Mode>,

    #[clap(long, value_enum)]
    quote: Option<QuoteFor>,

    #[clap(long)]
    force_absolute_path: bool,
}

fn main() -> Result<ExitCode> {
    // env_logger::init();
    let log_state = Arc::new(Mutex::new(LogWidgetState::default()));
    TuiLogger::init(LevelFilter::Info, log_state.clone()).expect("Could not init logger");
    let cli = Cli::parse();
    let here = fs::canonicalize(cli.start_path).context("start path")?;

    #[rustfmt::skip]
    let bindings = vec![
        (KeyModifiers::NONE, KeyCode::Enter, Action::Activate),
        (KeyModifiers::CONTROL, KeyCode::Enter, Action::AcceptCurrentDirectory),
        (KeyModifiers::NONE, KeyCode::Esc, Action::Abort),
        (KeyModifiers::CONTROL, KeyCode::Char('c'), Action::Abort),
        (KeyModifiers::NONE, KeyCode::Left, Action::Up),
        (KeyModifiers::NONE, KeyCode::Char('`'), Action::Up),
        (KeyModifiers::NONE, KeyCode::Right, Action::Down),
        (KeyModifiers::NONE, KeyCode::Up, Action::MoveCursor(-1)),
        (KeyModifiers::NONE, KeyCode::Down, Action::MoveCursor(1)),
        (KeyModifiers::NONE, KeyCode::PageDown, Action::MoveCursor(20)),
        (KeyModifiers::NONE, KeyCode::PageUp, Action::MoveCursor(-20)),
        (KeyModifiers::NONE, KeyCode::Home, Action::MoveCursor(isize::MIN)),
        (KeyModifiers::NONE, KeyCode::End, Action::MoveCursor(isize::MAX)),
        (KeyModifiers::SHIFT, KeyCode::PageDown, Action::MovePreview(20)),
        (KeyModifiers::SHIFT, KeyCode::PageUp, Action::MovePreview(-20)),
        (KeyModifiers::CONTROL, KeyCode::Up, Action::MovePreview(-20)),
        (KeyModifiers::CONTROL, KeyCode::Down, Action::MovePreview(20)),
        (KeyModifiers::NONE, KeyCode::Char('\\'), Action::CycleRecursion),
        (KeyModifiers::CONTROL, KeyCode::Char('h'), Action::Up),
        (KeyModifiers::CONTROL, KeyCode::Char('l'), Action::Down),
        (KeyModifiers::CONTROL, KeyCode::Char('j'), Action::MoveCursor(1)),
        (KeyModifiers::CONTROL, KeyCode::Char('k'), Action::MoveCursor(-1)),
        (KeyModifiers::CONTROL, KeyCode::Char('d'), Action::Home),
        (KeyModifiers::ALT | KeyModifiers::SHIFT, KeyCode::Char('A'), Action::CyclePalette),
        (KeyModifiers::CONTROL, KeyCode::Char('a'), Action::CycleHidden,),
        (KeyModifiers::CONTROL, KeyCode::Char('y'), Action::CycleIgnored,),
        (KeyModifiers::CONTROL, KeyCode::Char('f'), Action::CycleModeSkipping(vec![Mode::Mixed])),
        (KeyModifiers::ALT, KeyCode::Char('f'), Action::SetMode(Mode::Mixed)),
        (KeyModifiers::CONTROL, KeyCode::Char('e'), Action::Expand),
        (KeyModifiers::CONTROL, KeyCode::Char('r'), Action::CycleRecursion,),
        (KeyModifiers::CONTROL, KeyCode::Char('t'), Action::SetTarget),
        (KeyModifiers::CONTROL, KeyCode::Char('g'), Action::Open),
        (KeyModifiers::CONTROL, KeyCode::Char('p'), Action::TogglePreview),
        (KeyModifiers::ALT, KeyCode::Char('p'), Action::TogglePreviewMode),
        (KeyModifiers::ALT | KeyModifiers::SHIFT, KeyCode::Char('P'), Action::TogglePreviewColour),
        (KeyModifiers::CONTROL, KeyCode::Char('o'), Action::DirBack),
        (KeyModifiers::CONTROL, KeyCode::Char('u'), Action::DirForward),
    ];

    let mut app = App {
        dir_stack: DirStack::default(),
        read_opts: ReadOpts {
            target_dir: here.clone(),
            ..Default::default()
        },
        view_opts: ViewOpts {
            right_pane_mode: if cli.preview.unwrap_or(true) {
                RIGHT_PANE
            } else {
                RIGHT_PANE_HIDDEN
            },
            preview_mode_flag: PREVIEW_MODE,
            log_pane: cfg!(feature = "log_pane"),
            git_info: cfg!(feature = "git_info"),
            input_bottom: cfg!(feature = "input_bottom"),
        },
        result_opts: ResultOpts {
            force_absolute_path: cli.force_absolute_path,
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

    let (msg, code) = ratui::run(&mut store, &mut app, log_state)?;
    if let Some(msg) = msg {
        if let Some(path) = cli.output_path {
            let mut file = File::create(path)?;
            file.write_all(msg.as_bytes())?;
            file.flush()?;
        } else {
            let msg = match cli.quote {
                None => msg,
                Some(QuoteFor::Bash) => shell_quote::Bash::quote(&msg),
                Some(QuoteFor::Fish) => shell_quote::Fish::quote(&msg),
            };
            println!("{}", msg);
        }
    }
    Ok(code)
}
