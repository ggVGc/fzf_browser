use anyhow::Result;
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use std::io::stderr;
use std::sync::{Arc, Once};

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
