use std::collections::VecDeque;
use std::hash::Hash;

#[derive(Default)]
pub struct DirStack<T> {
    stack: VecDeque<T>,
    position: usize,
}

impl<T: Clone + Eq + Hash + std::fmt::Debug> DirStack<T> {
    pub fn push(&mut self, entry: T) {
        if self.stack.front() == Some(&entry) {
            return;
        }

        self.position = 0;
        self.stack.push_front(entry);
    }

    pub fn back(&mut self, entry: T) -> Option<T> {
        if self.stack.is_empty() {
            return None;
        }


        if self.position == 0 {
            self.stack.push_front(entry);
            self.dedup();
        }

        self.position = (self.position + 1).min(self.stack.len() - 1);
        self.stack.get(self.position).cloned()
    }

    pub fn forward(&mut self) -> Option<T> {
        if self.stack.is_empty() {
            return None;
        }

        self.position = self.position.saturating_sub(1);

        self.stack.get(self.position).cloned()
    }

    fn dedup(&mut self) {
        let mut seen = std::collections::HashSet::new();
        self.stack.retain(|entry| seen.insert(entry.clone()));
    }
}
