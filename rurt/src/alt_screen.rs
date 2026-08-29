use anyhow::Result;
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use std::cell::Cell;
use std::io::stderr;
use std::panic::AssertUnwindSafe;
use std::sync::{Arc, Once};

thread_local! {
    /// set while we're deliberately running code which is allowed to panic,
    /// e.g. third party image decoders; the hook stays out of the way so the
    /// tui isn't torn down for a panic we're about to handle ourselves
    static QUIET_PANIC: Cell<bool> = const { Cell::new(false) };
}

/// run `f`, catching any panic without restoring the terminal or printing
pub fn catch_quiet_panic<R>(f: impl FnOnce() -> R) -> std::thread::Result<R> {
    QUIET_PANIC.with(|q| q.set(true));
    let ret = std::panic::catch_unwind(AssertUnwindSafe(f));
    QUIET_PANIC.with(|q| q.set(false));
    ret
}

/// best effort description of a `catch_unwind` payload
pub fn panic_message(payload: &Box<dyn std::any::Any + Send>) -> String {
    if let Some(s) = payload.downcast_ref::<&str>() {
        (*s).to_string()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "panicked".to_string()
    }
}

pub fn enter_alt_screen() -> Result<DropRestore> {
    // copy-paste of ratatui::try_init() but for stderr
    enable_raw_mode()?;
    execute!(stderr(), EnterAlternateScreen)?;
    let restore = Arc::new(Restore {
        restore_once: Once::new(),
    });

    let for_hook = Arc::clone(&restore);
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |v| {
        if QUIET_PANIC.with(|q| q.get()) {
            return;
        }
        for_hook.restore();
        hook(v)
    }));

    Ok(DropRestore { restore })
}

pub struct Restore {
    restore_once: Once,
}

pub struct DropRestore {
    restore: Arc<Restore>,
}

impl Restore {
    pub fn restore(&self) {
        self.restore_once.call_once(|| {
            // copy-paste of ratatui::restore() but for stderr
            let _ = disable_raw_mode();
            let _ = execute!(stderr(), LeaveAlternateScreen);
        })
    }
}

impl Drop for DropRestore {
    fn drop(&mut self) {
        self.restore.restore()
    }
}
