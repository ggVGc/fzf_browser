use crate::action::{handle_action, matches_binding, Action, ActionResult};
use crate::alt_screen::enter_alt_screen;
use crate::draw::RightPane;
use crate::preview::Previews;
use crate::snapped::revalidate_cursor;
use crate::store::Store;
use crate::tui_log::LogWidgetState;
use crate::ui_state::{CommandPalette, Cursor, SortedItems, Ui};
use crate::{draw, filter_bindings, snapped, ui_state, App};
use anyhow::Result;
use crossterm::event;
use crossterm::event::{Event, KeyCode};
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
        cursor: Cursor::default(),
        cursor_showing: None,
        prompt: format!("{}> ", app.here.display()),
        active: true,
        sorted_items: SortedItems::default(),
        previews: Previews::default(),
        git_info: app.git_info(),
        preview_cursor: 0,
        preview_colours: true,
        ls_colors: LsColors::from_env().unwrap_or_default(),
        command_palette: CommandPalette::default(),
    };

    store.start_scan(app)?;

    loop {
        maybe_update_target_dir(app);

        store.nucleo.tick(10);

        ui.active = store.is_scanning() || ui.previews.is_scanning();

        if ui.active && (store.would_flicker() || ui.previews.would_flicker()) {
            for _ in 0..2 {
                event::poll(Duration::from_millis(20))?;
                store.nucleo.tick(10);
            }
        }

        let snap = store.nucleo.snapshot();

        let last_area = terminal
            .draw(|f| {
                let area = draw::setup_screen(f.area(), &app.view_opts);
                if app.view_opts.right_pane() == RightPane::Preview {
                    ui_state::fire_preview(&mut ui, app.view_opts.preview_mode(), area.side_pane);
                }

                let items_required = area.items_required(&app.view_opts);
                revalidate_cursor(&mut ui, snap, items_required);
                let items = snapped::ui_item_range(&mut ui, snap, items_required);
                draw::draw_ui(f, area, &ui, &app, &items, log_state.clone())
            })?
            .area;

        if !event::poll(Duration::from_millis(if ui.active { 5 } else { 90 }))? {
            continue;
        }

        let ev = event::read()?;

        let mut binding_action = match ev {
            Event::Key(key) => matches_binding(&app.bindings, key),
            _ => None,
        };

        if ui.command_palette.showing
            && ![Action::CyclePalette, Action::Abort]
                .map(|v| Some(v))
                .contains(&binding_action)
        {
            if matches!(ev, Event::Key(key) if key.code == KeyCode::Enter) {
                if let Some((_, _, action)) =
                    filter_bindings(&app.bindings, &ui.command_palette.input.value()).first()
                {
                    binding_action = Some(action.clone());
                }
                ui.command_palette.showing = false;
                ui.command_palette.input.reset();
            } else {
                if let Some(req) = to_input_request(&ev) {
                    ui.command_palette.input.handle(req);
                } else if ui.command_palette.input.value().is_empty() {
                    if let Some(action) = binding_action {
                        // why is this move?!
                        ui.command_palette.input = ui
                            .command_palette
                            .input
                            .with_value(action.name().to_string())
                    }
                }
                continue;
            }
        }

        match binding_action {
            Some(action) => {
                let action = handle_action(action, app, &mut ui)?;
                match action {
                    ActionResult::Ignored => (),
                    ActionResult::Configured => {
                        let next_screen = draw::setup_screen(last_area, &app.view_opts);
                        let items_required = next_screen.items_required(&app.view_opts);
                        revalidate_cursor(&mut ui, snap, items_required);
                        ui.preview_cursor = 0;

                        if app.view_opts.right_pane() == RightPane::Preview {
                            ui_state::fire_preview(
                                &mut ui,
                                app.view_opts.preview_mode(),
                                next_screen.side_pane,
                            );
                        }
                        reparse(store, &ui);
                    }

                    ActionResult::Navigated => {
                        app.read_opts.expansions.clear();
                        reparse(store, &ui);
                        ui.prompt = format!("{}> ", app.here.display());
                        ui.sorted_items.clear();
                        ui.git_info = app.git_info();
                        store.start_scan(app)?;
                    }

                    ActionResult::JustRescan => {
                        ui.sorted_items.clear();
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
