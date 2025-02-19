use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::item::Item;
use nucleo::Injector;

#[derive(Clone)]
pub struct AddItem {
    pub inner: Injector<Item>,
    pub cancelled: Arc<AtomicBool>,
}

impl AddItem {
    pub fn new(inner: Injector<Item>) -> Self {
        Self {
            inner,
            cancelled: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }

    pub fn send(&self, item: Item) -> std::result::Result<(), ()> {
        if self.cancelled.load(Ordering::Relaxed) {
            return Err(());
        }
        self.inner.push(item, |t, u| u[0] = t.text().into());
        Ok(())
    }
}
