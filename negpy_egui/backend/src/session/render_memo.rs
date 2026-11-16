//! Per-frame memo of the last displayed render, for instant frame switching.
//!
//! Mirrors `negpy/desktop/render_memo.py`. An entry is owned by the memo, so eviction,
//! replacement and invalidation drop the payload and release any resource it holds.

use std::collections::{HashMap, VecDeque};

use crate::domain::types::AppConfig;

/// The memo's slot budget. A large-entry memo uses the full-resolution preview budget.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MemoBudget {
    Standard,
    Large,
}

/// A bounded LRU keyed by source hash and, optionally, render variant.
pub struct RenderMemo<T> {
    app: AppConfig,
    keep_variants: bool,
    budget: MemoBudget,
    order: VecDeque<(String, String)>,
    entries: HashMap<(String, String), (String, T)>,
}

impl<T> RenderMemo<T> {
    /// A memo under `app`'s render budgets, keeping one entry per source hash.
    #[must_use]
    pub fn new(app: AppConfig) -> Self {
        Self::with_variants(app, false)
    }

    /// A memo that keeps several renders of one source hash, keyed by variant.
    #[must_use]
    pub fn with_variants(app: AppConfig, keep_variants: bool) -> Self {
        Self {
            app,
            keep_variants,
            budget: MemoBudget::Standard,
            order: VecDeque::new(),
            entries: HashMap::new(),
        }
    }

    /// Switch between the preview and full-resolution slot budgets.
    pub fn set_budget(&mut self, budget: MemoBudget) {
        self.budget = budget;
        self.trim();
    }

    fn entry_key(&self, file_hash: &str, memo_key: &str) -> (String, String) {
        (
            file_hash.to_string(),
            if self.keep_variants {
                memo_key.to_string()
            } else {
                String::new()
            },
        )
    }

    fn slot_limit(&self) -> usize {
        let configured = match self.budget {
            MemoBudget::Large => self.app.preview_cache_max_full_res_entries,
            MemoBudget::Standard => self.app.render_memo_max_entries,
        };
        usize::try_from(configured).unwrap_or(2).max(2)
    }

    fn touch(&mut self, key: &(String, String)) {
        if let Some(position) = self.order.iter().position(|candidate| candidate == key) {
            self.order.remove(position);
        }
        self.order.push_back(key.clone());
    }

    fn trim(&mut self) {
        let limit = self.slot_limit();
        while self.order.len() > limit {
            if let Some(key) = self.order.pop_front() {
                self.entries.remove(&key);
            }
        }
    }

    /// Store one render. Empty identifiers do not enter the memo.
    pub fn store(&mut self, file_hash: &str, memo_key: &str, payload: T) {
        if file_hash.is_empty() || memo_key.is_empty() {
            return;
        }
        let key = self.entry_key(file_hash, memo_key);
        self.entries
            .insert(key.clone(), (memo_key.to_string(), payload));
        self.touch(&key);
        self.trim();
    }

    /// Look up a render and make it the most recently used.
    pub fn get(&mut self, file_hash: &str, memo_key: &str) -> Option<&T> {
        let key = self.entry_key(file_hash, memo_key);
        if self
            .entries
            .get(&key)
            .is_none_or(|(stored_key, _)| stored_key != memo_key)
        {
            return None;
        }
        self.touch(&key);
        self.entries.get(&key).map(|(_, payload)| payload)
    }

    /// Peek without changing the LRU order.
    #[must_use]
    pub fn peek(&self, file_hash: &str, memo_key: &str) -> Option<&T> {
        let key = self.entry_key(file_hash, memo_key);
        self.entries
            .get(&key)
            .filter(|(stored_key, _)| stored_key == memo_key)
            .map(|(_, payload)| payload)
    }

    /// Follow a render-neutral identity change, such as bounds written back after a render.
    pub fn rekey(&mut self, file_hash: &str, new_key: &str, old_key: &str) {
        if new_key.is_empty() || (self.keep_variants && old_key.is_empty()) {
            return;
        }
        let from = self.entry_key(file_hash, old_key);
        let Some((_, payload)) = self.entries.remove(&from) else {
            return;
        };
        if let Some(position) = self.order.iter().position(|candidate| candidate == &from) {
            self.order.remove(position);
        }
        self.store(file_hash, new_key, payload);
    }

    /// Drop every variant of one source hash.
    pub fn invalidate(&mut self, file_hash: &str) {
        self.order.retain(|(hash, _)| hash != file_hash);
        self.entries.retain(|(hash, _), _| hash != file_hash);
    }

    /// Drop every entry.
    pub fn clear(&mut self) {
        self.order.clear();
        self.entries.clear();
    }

    /// The number of stored renders.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether no render is stored.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}
