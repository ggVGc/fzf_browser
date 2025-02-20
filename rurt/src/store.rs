use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::thread::JoinHandle;

use crate::fuzz::AddItem;
use crate::item::Item;
use crate::walk::stream_content;
use crate::App;
use anyhow::Result;
use nucleo::Nucleo;

pub struct Store {
    pub nucleo: Nucleo<Item>,
    current_scan: Option<CurrentScan>,
}

pub struct CurrentScan {
    handle: JoinHandle<Result<()>>,
    cancellation: Arc<AtomicBool>,
}

impl Store {
    pub fn new(nucleo: Nucleo<Item>) -> Self {
        Self {
            nucleo,
            current_scan: None,
        }
    }

    pub fn start_scan(&mut self, app: &App) -> Result<()> {
        self.cancel_scan()?;

        let cancellation = Arc::new(AtomicBool::new(false));

        let tx = AddItem {
            inner: self.nucleo.injector(),
            cancelled: cancellation.clone(),
        };

        let here = app.here.to_path_buf();
        let read_opts = app.read_opts.clone();

        let handle = std::thread::spawn(move || stream_content(tx, here, &read_opts));

        self.current_scan = Some(CurrentScan {
            handle,
            cancellation,
        });

        Ok(())
    }

    pub fn is_scanning(&self) -> bool {
        self.current_scan
            .as_ref()
            .map(|scan| !scan.handle.is_finished())
            .unwrap_or(false)
    }

    pub fn cancel_scan(&mut self) -> Result<()> {
        if let Some(scan) = self.current_scan.take() {
            scan.cancellation
                .store(true, std::sync::atomic::Ordering::Relaxed);
            scan.handle.join().expect("thread panic")?;
        }
        self.nucleo.restart(true);

        Ok(())
    }
}
