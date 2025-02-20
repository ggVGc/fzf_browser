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
    SetTarget,
    Expand,
    Open,
    DirBack,
    DirForward,
    Abort,
}

pub enum ActionResult {
    Ignored,
    Navigated,
    Configured,
    Exit(Option<String>, ExitCode),
}

pub fn handle_action(
    action: Action,
    app: &mut App,
    ui: &mut Ui,
    snap: &Snapshot<Item>,
) -> anyhow::Result<ActionResult> {
    let here = &mut app.here;
    let read_opts = &mut app.read_opts;
    let dir_stack = &mut app.dir_stack;

    let item = item_range(snap, ui.cursor, ui.cursor + 1, ui.input.value().is_empty()).pop();

    Ok(match action {
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
        Action::MoveCursor(delta) => {
            ui.cursor = u32::try_from((ui.cursor as i32) + delta).unwrap_or(0);
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
        Action::Abort => ActionResult::Exit(None, ExitCode::FAILURE),
        Action::Default => {
            if let Some(Item::FileEntry { name, .. }) = item {
                if let Ok(cand) = ensure_directory(here.join(name)) {
                    dir_stack.push(here.to_path_buf());
                    *here = cand;
                    ActionResult::Navigated
                } else {
                    ActionResult::Exit(
                        Some(here.join(name).display().to_string()),
                        ExitCode::SUCCESS,
                    )
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
