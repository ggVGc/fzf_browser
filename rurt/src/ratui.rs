use crate::action::{handle_action, matches_binding, ActionResult};
use crate::snapped::item_under_cursor;
use crate::store::Store;
use crate::tui_log::LogWidgetState;
use crate::ui_state::Ui;
use crate::{draw, snapped, ui_state, App};
use anyhow::Result;
use crossterm::event::Event;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::{event, execute};
use lscolors::LsColors;
use nucleo::pattern::{CaseMatching, Normalization};
use ratatui::prelude::*;
use std::collections::VecDeque;
use std::io::stderr;
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use tui_input::backend::crossterm::to_input_request;
use tui_input::Input;

pub fn run(
    store: &mut Store,
    app: &mut App,
    log_state: Arc<Mutex<LogWidgetState>>,
) -> Result<(Option<String>, ExitCode)> {
    // copy-paste of ratatui::try_init() but for stderr
    enable_raw_mode()?;
    execute!(stderr(), EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stderr());
    let mut terminal = Terminal::new(backend)?;
    let _restore = DropRestore {};

    let mut ui = Ui {
        input: Input::default(),
        view_start: 0,
        cursor: 0,
        cursor_showing: Some(app.here.to_path_buf()),
        prompt: format!("{}> ", app.here.display()),
        active: true,
        sorted_items: Vec::new(),
        sorted_until: 0,
        previews: VecDeque::new(),
        preview_colours: true,
        ls_colors: LsColors::from_env().unwrap_or_default(),
    };

    store.start_scan(app)?;

    loop {
        maybe_update_target_dir(app);

        store.nucleo.tick(10);

        let snap = store.nucleo.snapshot();

        ui.active = store.is_scanning() || ui.previews.iter().any(|v| !v.worker.is_finished());

        if ui.active && ui.previews.iter().any(|v| ui_state::would_flicker(v)) {
            event::poll(Duration::from_millis(60))?;
            thread::yield_now();
        }

        let last_area = terminal
            .draw(|f| {
                let area = draw::setup_screen(f.area());
                ui_state::fire_preview(&mut ui, area.right_pane);
                let item_area = area.left_pane;
                snapped::revalidate_cursor(&mut ui, snap, item_area);
                let items = snapped::ui_item_range(&mut ui, snap, item_area);
                draw::draw_ui(f, area, &ui, &items, log_state.clone())
            })?
            .area;

        if !event::poll(Duration::from_millis(if ui.active { 5 } else { 90 }))? {
            continue;
        }

        let ev = event::read()?;

        let binding_action = match ev {
            Event::Key(key) => matches_binding(&app.bindings, key),
            _ => None,
        };

        match binding_action {
            Some(action) => {
                let action = handle_action(action, app, &mut ui)?;
                match action {
                    ActionResult::Ignored => (),
                    ActionResult::Configured => {
                        ui.cursor_showing = item_under_cursor(&mut ui, snap).map(PathBuf::from);
                        ui_state::fire_preview(&mut ui, draw::setup_screen(last_area).right_pane);
                    }

                    ActionResult::Navigated => {
                        app.read_opts.expansions.clear();
                        ui.prompt = format!("{}> ", app.here.display());
                        ui.sorted_items.clear();
                        ui.sorted_until = 0;
                        store.start_scan(app)?;
                    }

                    ActionResult::JustRescan => {
                        ui.sorted_items.clear();
                        ui.sorted_until = 0;
                        store.start_scan(app)?;
                    }

                    ActionResult::Exit(msg, code) => return Ok((msg, code)),
                }
            }
            None => {
                if let Some(req) = to_input_request(&ev) {
                    if ui.input.handle(req).map(|v| v.value).unwrap_or_default() {
                        store.nucleo.pattern.reparse(
                            0,
                            ui.input.value(),
                            CaseMatching::Smart,
                            Normalization::Smart,
                            false,
                        );
                    }
                }
            }
        }
    }
}

struct DropRestore {}
impl Drop for DropRestore {
    fn drop(&mut self) {
        // copy-paste of ratatui::restore() but for stderr
        let _ = disable_raw_mode();
        let _ = execute!(stderr(), LeaveAlternateScreen);
    }
}

fn maybe_update_target_dir(app: &mut App) {
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
}
