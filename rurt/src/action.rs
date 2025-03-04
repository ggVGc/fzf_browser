use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use crate::item::Item;
use crate::ratui::{item_range, Ui};
use crate::walk::{MODES, RECURSION};
use crate::App;
use anyhow::{anyhow, bail};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use nucleo::Snapshot;

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Action {
    Default,
    Ignore,
    Up,
    Down,
    Home,
    // positive is *flips coin* towards the bottom of the screen
    MoveCursor(i32),
    CycleHidden,
    CycleIgnored,
    CycleMode,
    CycleRecursion,
    TogglePreview,
    TogglePreviewColour,
    SetTarget,
    Expand,
    Open,
    DirBack,
    DirForward,
    Abort,
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

pub fn item_under_cursor<'s>(ui: &mut Ui, snap: &'s Snapshot<Item>) -> Option<&'s Path> {
    item_range(snap, ui.cursor, ui.cursor + 1, ui)
        .pop()
        .and_then(|it| it.path())
}

pub fn handle_action(action: Action, app: &mut App, ui: &mut Ui) -> anyhow::Result<ActionResult> {
    let here = &mut app.here;
    let read_opts = &mut app.read_opts;
    let dir_stack = &mut app.dir_stack;

    Ok(match action {
        Action::Up => {
            dir_stack.push(here.clone());
            here.pop();
            ActionResult::Navigated
        }
        Action::Down => {
            if let Some(cand) = ui
                .cursor_showing
                .as_ref()
                .and_then(|name| ensure_directory(here.join(name)).ok())
            {
                dir_stack.push(here.clone());
                *here = cand;
                ActionResult::Navigated
            } else {
                ActionResult::Ignored
            }
        }
        Action::MoveCursor(delta) => {
            ui.cursor = u32::try_from((ui.cursor as i32).saturating_add(delta)).unwrap_or(0);
            ActionResult::Configured
        }
        Action::Home => {
            dir_stack.push(here.clone());
            *here =
                dirs::home_dir().ok_or_else(|| anyhow!("but you don't even have a home dir"))?;
            ActionResult::Navigated
        }
        Action::CycleHidden => {
            read_opts.show_hidden = !read_opts.show_hidden;
            ActionResult::Navigated
        }
        Action::CycleIgnored => {
            read_opts.show_ignored = !read_opts.show_ignored;
            ActionResult::Navigated
        }
        Action::CycleMode => {
            read_opts.mode_index = (read_opts.mode_index + 1) % MODES.len();
            ActionResult::Navigated
        }
        Action::CycleRecursion => {
            read_opts.recursion_index = (read_opts.recursion_index + 1) % RECURSION.len();
            ActionResult::Navigated
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
        Action::TogglePreviewColour => {
            ui.preview_colours = !ui.preview_colours;
            ActionResult::Configured
        }
        Action::SetTarget => {
            read_opts.target_dir.clone_from(here);
            ActionResult::Configured
        }
        Action::Open => {
            if let Some(showing) = &ui.cursor_showing {
                open::that_detached(here.join(showing))?;
            }
            ActionResult::Ignored
        }
        Action::Expand => {
            if let Some(name) = &ui.cursor_showing {
                read_opts.expansions.push(here.join(name));
                ActionResult::JustRescan
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
        Action::Default => {
            if let Some(name) = &ui.cursor_showing {
                if let Ok(cand) = ensure_directory(here.join(name)) {
                    dir_stack.push(here.to_path_buf());
                    *here = cand;
                    ActionResult::Navigated
                } else {
                    let mut cand = here.join(name);
                    if let Ok(cwd) = std::env::current_dir() {
                        if let Ok(stripped) = cand.strip_prefix(&cwd) {
                            cand = stripped.to_path_buf();
                        }
                    }
                    ActionResult::Exit(Some(cand.display().to_string()), ExitCode::SUCCESS)
                }
            } else {
                ActionResult::Exit(None, ExitCode::FAILURE)
            }
        }
        Action::Ignore => ActionResult::Ignored,
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
            Some(*action)
        } else {
            None
        }
    })
}
