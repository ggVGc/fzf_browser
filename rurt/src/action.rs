use std::borrow::Cow;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use crate::draw::RightPane;
use crate::ui_state::{matching_preview, Ui};
use crate::walk::{Mode, MODES};
use crate::App;
use anyhow::{anyhow, bail};
use convert_case::{Case, Casing};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Action {
    Activate,
    AcceptCurrentDirectory,
    AcceptSelectedItems,
    Ignore,
    Up,
    Down,
    Home,
    // positive is *flips coin* towards the bottom of the screen
    MoveCursor(isize),
    MovePreview(isize),
    CyclePalette,
    CycleHidden,
    CycleIgnored,
    CycleModeSkipping(Vec<Mode>),
    SetMode(Mode),
    CycleRecursion,
    TogglePreview,
    TogglePreviewMode,
    TogglePreviewColour,
    ToggleSelection,
    SetTarget,
    Expand,
    Open,
    FocusGit,
    DirBack,
    DirForward,
    Abort,
}

impl Action {
    pub fn name(&self) -> Cow<'static, str> {
        match self {
            Action::MoveCursor(delta) => format!("move cursor {}", show_delta(*delta)).into(),
            Action::MovePreview(delta) => format!("move preview {}", show_delta(*delta)).into(),
            other => format!("{:?}", other).to_case(Case::Lower).into(),
        }
    }
}

fn show_delta(delta: isize) -> Cow<'static, str> {
    match delta {
        isize::MIN => return "to start".into(),
        isize::MAX => return "to end".into(),
        _ => (),
    }

    if delta < 0 {
        format!("up by {}", -delta).into()
    } else {
        format!("down by {}", delta).into()
    }
}

pub enum ActionResult {
    /// didn't do anything relevant
    Ignored,
    /// changed 'here', clear state and rescan
    Navigated,
    /// changed some scan state, rescan but don't clear
    JustRescan,
    /// changed some config state, don't rescan
    Configured,
    /// we're done, print something and bail
    Exit(Option<String>, ExitCode),
}

pub fn handle_action(action: Action, app: &mut App, ui: &mut Ui) -> anyhow::Result<ActionResult> {
    let here = &mut app.here;
    let read_opts = &mut app.read_opts;
    let view_opts = &mut app.view_opts;
    let dir_stack = &mut app.dir_stack;

    Ok(match action {
        Action::Up => {
            ui.input.reset();
            dir_stack.push(here.clone());
            here.pop();
            ActionResult::Navigated
        }
        Action::Down => {
            if let Some(cand) = get_cursor_directory(here, ui) {
                ui.input.reset();
                dir_stack.push(here.clone());
                *here = cand;
                ActionResult::Navigated
            } else {
                ActionResult::Ignored
            }
        }
        Action::MoveCursor(delta) => {
            ui.cursor.pending_move = Some(delta);
            ActionResult::Configured
        }
        Action::MovePreview(delta) => {
            let max_cursor = matching_preview(ui, view_opts.preview_mode())
                .and_then(|p| {
                    p.data
                        .lock()
                        .ok()
                        .and_then(|d| d.render.as_ref().map(|r| r.lines.len()))
                })
                .unwrap_or(usize::MAX);

            ui.preview_cursor =
                usize::try_from((ui.preview_cursor as isize).saturating_add(delta as isize))
                    .unwrap_or(0)
                    .min(max_cursor);

            // 'Configured' resets the preview_cursor
            ActionResult::Ignored
        }
        Action::Home => {
            dir_stack.push(here.clone());
            *here =
                dirs::home_dir().ok_or_else(|| anyhow!("but you don't even have a home dir"))?;
            ActionResult::Navigated
        }
        Action::CyclePalette => {
            ui.command_palette.showing = !ui.command_palette.showing;
            ActionResult::Ignored
        }
        Action::CycleHidden => {
            read_opts.show_hidden = !read_opts.show_hidden;
            ActionResult::Navigated
        }
        Action::CycleIgnored => {
            read_opts.show_ignored = !read_opts.show_ignored;
            ActionResult::Navigated
        }
        Action::CycleModeSkipping(skipped_modes) => {
            let mut new_index = (read_opts.mode_index + 1) % MODES.len();
            let mut skipped_indices = vec![];
            for skipped in skipped_modes {
                skipped_indices.push(MODES.iter().position(|m| *m == skipped).unwrap());
            }

            while skipped_indices.contains(&new_index) {
                new_index = (new_index + 1) % MODES.len();
            }

            read_opts.mode_index = new_index;

            ActionResult::Navigated
        }
        Action::SetMode(mode) => {
            read_opts.mode_index = MODES.iter().position(|m| *m == mode).unwrap();
            ActionResult::Navigated
        }
        Action::FocusGit => {
            let i = view_opts
                .right_pane_mode
                .iter()
                .position(|p| *p == RightPane::InteractiveGitLog)
                .expect("git log mode present");
            view_opts.right_pane_mode.rotate_left(i);
            ui.bad_git_log.focus = true;
            ActionResult::Ignored
        }
        Action::CycleRecursion => {
            read_opts.recursion = read_opts.recursion.next();
            ActionResult::Navigated
        }
        Action::TogglePreview => {
            view_opts.right_pane_mode.rotate_left(1);
            ActionResult::Configured
        }
        Action::TogglePreviewMode => {
            view_opts.preview_mode_flag.rotate_left(1);
            ActionResult::Configured
        }
        Action::TogglePreviewColour => {
            ui.preview_colours = !ui.preview_colours;
            ActionResult::Configured
        }
        Action::ToggleSelection => {
            ui.toggle_selection();
            handle_action(Action::MoveCursor(1), app, ui)?
        }
        Action::SetTarget => {
            read_opts.target_dir.clone_from(here);
            ActionResult::Configured
        }
        Action::Open => {
            if let Some(showing) = ui.cursor_showing_path() {
                open::that_detached(here.join(showing))?;
            }
            ActionResult::Ignored
        }
        Action::Expand => {
            if let Some(name) = ui.cursor_showing_path() {
                if read_opts.expansions.insert(here.join(name)) {
                    ActionResult::JustRescan
                } else {
                    ActionResult::Ignored
                }
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
        Action::Abort => ActionResult::Exit(None, ExitCode::FAILURE),
        Action::Activate => {
            if let Some(name) = ui.cursor_showing_path() {
                if let Ok(cand) = ensure_directory(here.join(name)) {
                    ui.input.reset();
                    dir_stack.push(here.to_path_buf());
                    *here = cand;
                    ActionResult::Navigated
                } else {
                    if !ui.selected_items.is_empty() {
                        handle_action(Action::ToggleSelection, app, ui)?
                    } else {
                        let mut cand = here.join(name);
                        if !app.result_opts.force_absolute_path {
                            if let Ok(cwd) = std::env::current_dir() {
                                if let Ok(stripped) = cand.strip_prefix(&cwd) {
                                    cand = stripped.to_path_buf();
                                }
                            }
                        }
                        ActionResult::Exit(Some(cand.display().to_string()), ExitCode::SUCCESS)
                    }
                }
            } else {
                ActionResult::Exit(None, ExitCode::FAILURE)
            }
        }
        Action::AcceptSelectedItems => {
            if !ui.selected_items.is_empty() {
                let selected_paths: Vec<String> = ui
                    .selected_items
                    .iter()
                    .map(|path| {
                        let mut cand = path.clone();
                        if !app.result_opts.force_absolute_path {
                            if let Ok(cwd) = std::env::current_dir() {
                                if let Ok(stripped) = cand.strip_prefix(&cwd) {
                                    cand = stripped.to_path_buf();
                                }
                            }
                        }
                        cand.display().to_string()
                    })
                    .collect();
                ActionResult::Exit(Some(selected_paths.join("\n")), ExitCode::SUCCESS)
            } else {
                ActionResult::Ignored
            }
        }
        Action::AcceptCurrentDirectory => {
            let mut cand = here.clone();
            if let Ok(cwd) = std::env::current_dir() {
                if cwd == cand {
                    cand = ".".into()
                } else if let Ok(stripped) = cand.strip_prefix(&cwd) {
                    cand = stripped.to_path_buf();
                }
            }
            ActionResult::Exit(Some(cand.display().to_string()), ExitCode::SUCCESS)
        }
        Action::Ignore => ActionResult::Ignored,
    })
}

fn get_cursor_directory(current_dir: &PathBuf, ui: &Ui) -> Option<PathBuf> {
    ui.cursor_showing_path().and_then(|name| {
        let path = current_dir.clone().join(name);
        ensure_directory(path.clone()).ok().or_else(|| {
            let path = path.as_path().parent()?;
            Some(path.into())
        })
    })
}

fn ensure_directory(p: impl AsRef<Path>) -> anyhow::Result<PathBuf> {
    let canon = fs::canonicalize(p)?;
    if !fs::metadata(&canon)?.is_dir() {
        bail!("resolved path is not a directory");
    }

    Ok(canon)
}

pub fn matches_binding(
    bindings: &[(KeyModifiers, KeyCode, Action)],
    final_key: KeyEvent,
) -> Option<Action> {
    bindings.iter().find_map(|(modifier, key, action)| {
        if final_key.code == *key && final_key.modifiers == *modifier {
            Some(action.clone())
        } else {
            None
        }
    })
}
