use std::collections::HashMap;
use std::thread::JoinHandle;

pub struct Cache<K, V> {
    map: HashMap<K, Entry<V>>,
}

struct Entry<V> {
    handle: Option<JoinHandle<Option<V>>>,
    value: Option<V>,
}

impl<K, V> Cache<K, V>
where
    K: Eq + std::hash::Hash + Clone,
    V: Send + 'static,
{
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }

    pub fn compute(
        &mut self,
        key: K,
        f: impl FnOnce() -> Option<V> + Send + 'static,
    ) -> Option<&V> {
        let entry = self.map.entry(key.clone()).or_insert_with(|| {
            let handle = std::thread::spawn(f);
            Entry {
                handle: Some(handle),
                value: None,
            }
        });

        if entry
            .handle
            .as_mut()
            .map(|h| h.is_finished())
            .unwrap_or_default()
        {
            entry.value = entry
                .handle
                .take()
                .expect("just checked")
                .join()
                .ok()
                .flatten();
        }

        entry.value.as_ref()
    }
}
