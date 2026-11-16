//! Wall time per pipeline step, collected per thread and logged as one line that adds up.
//!
//! Mirrors `negpy/kernel/system/timing.py`: a thread starts a tree with [`collect`],
//! records laps and nested steps inside it, and formats the result with [`summary`].

use std::cell::RefCell;
use std::time::Instant;

/// A step below this is a cache hit or a no-op, and only clutters the line.
const SHOWN_MS: f64 = 0.5;

/// One timed node: its own name, its total and the nodes under it.
#[derive(Clone, Debug, PartialEq)]
pub struct Step {
    pub name: String,
    pub ms: f64,
    pub children: Vec<Step>,
    /// When the previous lap or child step ended.
    last: Instant,
}

impl Step {
    fn new(name: &str, last: Instant) -> Self {
        Self {
            name: name.to_string(),
            ms: 0.0,
            children: Vec::new(),
            last,
        }
    }
}

thread_local! {
    static CURRENT: RefCell<Vec<Step>> = const { RefCell::new(Vec::new()) };
}

/// An open timing tree. [`Collector::finish`] closes it and returns the root; dropping
/// it unclosed discards the tree.
pub struct Collector {
    start: Option<Instant>,
}

impl Collector {
    /// Close the tree and return its root, with the total set from the collector's start.
    #[must_use]
    pub fn finish(mut self) -> Step {
        let elapsed = self
            .start
            .take()
            .map_or(0.0, |start| start.elapsed().as_secs_f64() * 1000.0);
        CURRENT.with(|steps| {
            let mut steps = steps.borrow_mut();
            let mut root = steps
                .pop()
                .unwrap_or_else(|| Step::new("total", Instant::now()));
            root.ms = elapsed;
            root
        })
    }

    /// The open tree on this thread, as a snapshot.
    #[must_use]
    pub fn root(&self) -> Option<Step> {
        current()
    }
}

impl Drop for Collector {
    fn drop(&mut self) {
        if self.start.take().is_some() {
            CURRENT.with(|steps| {
                let _ = steps.borrow_mut().pop();
            });
        }
    }
}

/// Start a timing tree named `name`. A thread without one records nothing.
#[must_use]
pub fn collect(name: &str) -> Collector {
    let now = Instant::now();
    CURRENT.with(|steps| steps.borrow_mut().push(Step::new(name, now)));
    Collector { start: Some(now) }
}

/// A snapshot of the open tree on this thread.
#[must_use]
pub fn current() -> Option<Step> {
    CURRENT.with(|steps| steps.borrow().last().cloned())
}

/// A child step whose own laps nest under it.
pub struct StepGuard {
    start: Instant,
    opened: bool,
}

impl Drop for StepGuard {
    fn drop(&mut self) {
        if !self.opened {
            return;
        }
        let now = Instant::now();
        let elapsed = (now - self.start).as_secs_f64() * 1000.0;
        CURRENT.with(|steps| {
            let mut steps = steps.borrow_mut();
            let Some(finished) = steps.pop() else {
                return;
            };
            if let Some(parent) = steps.last_mut() {
                let mut finished = finished;
                finished.ms = elapsed;
                parent.children.push(finished);
                parent.last = now;
            }
        });
    }
}

/// Open a child step. A thread without a collector records nothing.
#[must_use]
pub fn step(name: &str) -> StepGuard {
    let opened = CURRENT.with(|steps| {
        let mut steps = steps.borrow_mut();
        if steps.is_empty() {
            return false;
        }
        let now = Instant::now();
        steps.push(Step::new(name, now));
        true
    });
    StepGuard {
        start: Instant::now(),
        opened,
    }
}

/// Record the time since the previous lap or step as `name`.
pub fn lap(name: &str) {
    CURRENT.with(|steps| {
        let mut steps = steps.borrow_mut();
        let Some(node) = steps.last_mut() else {
            return;
        };
        let now = Instant::now();
        let ms = (now - node.last).as_secs_f64() * 1000.0;
        node.children.push(Step {
            name: name.to_string(),
            ms,
            children: Vec::new(),
            last: now,
        });
        node.last = now;
    });
}

/// Same-named nodes summed in first-seen order, so a per-tile step reads as one entry.
fn merged(children: &[Step]) -> Vec<Step> {
    let mut out: Vec<Step> = Vec::new();
    for child in children {
        if let Some(existing) = out.iter_mut().find(|node| node.name == child.name) {
            existing.ms += child.ms;
            existing.children.extend(child.children.iter().cloned());
        } else {
            out.push(Step {
                name: child.name.clone(),
                ms: child.ms,
                children: child.children.clone(),
                last: child.last,
            });
        }
    }
    out
}

fn parts(node: &Step) -> Vec<String> {
    let mut out = Vec::new();
    for child in merged(&node.children) {
        if child.ms < SHOWN_MS {
            continue;
        }
        let nested = parts(&child);
        let mut text = format!("{} {:.0}", child.name, child.ms);
        if !nested.is_empty() {
            text.push_str(&format!(" [{}]", nested.join(", ")));
        }
        out.push(text);
    }
    let rest = node.ms - node.children.iter().map(|child| child.ms).sum::<f64>();
    if !node.children.is_empty() && rest >= SHOWN_MS {
        out.push(format!("other {rest:.0}"));
    }
    out
}

/// `60 ms = decode 10 + GPU 40 [upload 30, other 10] + other 8`; all figures in ms.
#[must_use]
pub fn summary(root: &Step) -> String {
    let parts = parts(root);
    if parts.is_empty() {
        format!("{:.0} ms", root.ms)
    } else {
        format!("{:.0} ms = {}", root.ms, parts.join(" + "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nothing_is_recorded_without_a_collector() {
        lap("orphan");
        {
            let _guard = step("orphan step");
            lap("inner");
        }
        assert!(current().is_none());
    }

    #[test]
    fn an_empty_tree_reads_its_total() {
        let collector = collect("total");
        std::thread::sleep(std::time::Duration::from_millis(2));
        let root = collector.finish();
        assert!(root.ms >= 2.0);
        assert_eq!(summary(&root), format!("{:.0} ms", root.ms));
    }

    #[test]
    fn laps_and_steps_add_up_to_the_summary() {
        let collector = collect("total");
        std::thread::sleep(std::time::Duration::from_millis(2));
        lap("decode");
        {
            let _gpu = step("GPU");
            std::thread::sleep(std::time::Duration::from_millis(4));
            lap("upload");
        }
        lap("encode");
        let root = collector.finish();
        let text = summary(&root);
        assert!(text.contains("GPU 4"), "{text}");
        assert!(text.contains("upload"), "{text}");
    }

    #[test]
    fn repeated_steps_merge_into_one_entry() {
        let collector = collect("total");
        for _ in 0..2 {
            let _tile = step("tile");
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        let root = collector.finish();
        let text = summary(&root);
        assert_eq!(text.matches("tile").count(), 1, "{text}");
    }
}
