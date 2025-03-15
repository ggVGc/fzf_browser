use crate::action::{handle_action, matches_binding, ActionResult};
use crate::alt_screen::enter_alt_screen;
use crate::draw::RightPane;
use crate::preview::Previews;
use crate::snapped::item_under_cursor;
use crate::store::Store;
use crate::tui_log::LogWidgetState;
use crate::ui_state::Ui;
use crate::{draw, snapped, ui_state, App};
use anyhow::Result;
use crossterm::event;
use crossterm::event::Event;
use lscolors::LsColors;
use nucleo::pattern::{CaseMatching, Normalization};
use ratatui::prelude::*;
use std::io::stderr;
use std::process::ExitCode;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tui_input::backend::crossterm::to_input_request;
use tui_input::Input;

pub fn run(
    store: &mut Store,
    app: &mut App,
    log_state: Arc<Mutex<LogWidgetState>>,
) -> Result<(Option<String>, ExitCode)> {
    let _restore_on_drop = enter_alt_screen()?;
    let backend = CrosstermBackend::new(stderr());
    let mut terminal = Terminal::new(backend)?;

    let mut ui = Ui {
        boot: Instant::now(),
        input: Input::default(),
        view_start: 0,
        cursor: 0,
        cursor_showing: None,
        prompt: format!("{}> ", app.here.display()),
        active: true,
        sorted_items: Vec::new(),
        sorted_until: 0,
        previews: Previews::default(),
        preview_cursor: 0,
        preview_colours: true,
        ls_colors: LsColors::from_env().unwrap_or_default(),
    };

    store.start_scan(app)?;

    loop {
        maybe_update_target_dir(app);

        store.nucleo.tick(10);

        ui.active = store.is_scanning() || ui.previews.is_scanning();

        if ui.active && (store.would_flicker() || ui.previews.would_flicker()) {
            event::poll(Duration::from_millis(50))?;
            store.nucleo.tick(10);
        }

        let snap = store.nucleo.snapshot();

        let last_area = terminal
            .draw(|f| {
                let area = draw::setup_screen(f.area(), &app.view_opts);
                if app.view_opts.right_pane() == RightPane::Preview {
                    ui_state::fire_preview(&mut ui, area.side_pane);
                }

                let mut item_area = area.main_pane;
                if app.view_opts.right_pane() == RightPane::SecondListing {
                    item_area.height *= 2;
                }
                snapped::revalidate_cursor(&mut ui, snap, item_area);
                let items = snapped::ui_item_range(&mut ui, snap, item_area);
                draw::draw_ui(f, area, &ui, &app.view_opts, &items, log_state.clone())
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
                        ui.cursor_showing = item_under_cursor(&mut ui, snap).cloned();
                        ui.preview_cursor = 0;
                        if app.view_opts.right_pane() == RightPane::Preview {
                            ui_state::fire_preview(
                                &mut ui,
                                draw::setup_screen(last_area, &app.view_opts).side_pane,
                            );
                        }
                        reparse(store, &ui);
                    }

                    ActionResult::Navigated => {
                        app.read_opts.expansions.clear();
                        reparse(store, &ui);
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
                        reparse(store, &ui);
                    }
                }
            }
        }
    }
}

fn reparse(store: &mut Store, ui: &Ui) {
    store.nucleo.pattern.reparse(
        0,
        ui.input.value(),
        CaseMatching::Smart,
        Normalization::Smart,
        false,
    );
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
