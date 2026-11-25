//! In-memory state for mission runs, SSE subscribers and exports.
//!
//! Runs are bounded: at most 16 are retained and each has at most 2,000 stored
//! events and 32 subscribers. Results and exports are strings/values produced by
//! the simulation; nothing is synthesized.

use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};

use crate::json::Value;

pub const MAX_RUNS: usize = 16;
pub const MAX_EVENTS: usize = 2_000;
pub const MAX_SUBSCRIBERS: usize = 32;

#[derive(Debug, Clone, PartialEq)]
pub enum RunStatus {
    Running,
    Complete,
    Failed(String),
}

impl RunStatus {
    pub fn as_str(&self) -> &str {
        match self {
            RunStatus::Running => "running",
            RunStatus::Complete => "complete",
            RunStatus::Failed(_) => "failed",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SseEvent {
    pub name: String,
    pub data: String,
}

pub struct RunRecord {
    pub id: u64,
    status: Mutex<RunStatus>,
    events: Mutex<Vec<SseEvent>>,
    subscribers: Mutex<Vec<Sender<SseEvent>>>,
    result: Mutex<Option<Value>>,
    csv: Mutex<Option<String>>,
    report: Mutex<Option<String>>,
    summary: Mutex<Option<String>>,
}

impl RunRecord {
    fn new(id: u64) -> Self {
        Self {
            id,
            status: Mutex::new(RunStatus::Running),
            events: Mutex::new(Vec::new()),
            subscribers: Mutex::new(Vec::new()),
            result: Mutex::new(None),
            csv: Mutex::new(None),
            report: Mutex::new(None),
            summary: Mutex::new(None),
        }
    }

    pub fn status(&self) -> RunStatus {
        self.status
            .lock()
            .map(|s| s.clone())
            .unwrap_or_else(|_| RunStatus::Failed("state poisoned".into()))
    }

    pub fn result(&self) -> Option<Value> {
        self.result.lock().ok().and_then(|r| r.clone())
    }

    pub fn csv(&self) -> Option<String> {
        self.csv.lock().ok().and_then(|r| r.clone())
    }

    pub fn report(&self) -> Option<String> {
        self.report.lock().ok().and_then(|r| r.clone())
    }

    pub fn summary(&self) -> Option<String> {
        self.summary.lock().ok().and_then(|r| r.clone())
    }

    pub fn emit(&self, name: impl Into<String>, data: impl Into<String>) {
        let event = SseEvent {
            name: name.into(),
            data: data.into(),
        };
        if let Ok(mut events) = self.events.lock() {
            if events.len() < MAX_EVENTS {
                events.push(event.clone());
            }
        }
        if let Ok(mut subscribers) = self.subscribers.lock() {
            subscribers.retain(|sender| sender.send(event.clone()).is_ok());
        }
    }

    pub fn subscribe(&self) -> Option<(Vec<SseEvent>, Receiver<SseEvent>)> {
        let past = self.events.lock().ok()?.clone();
        let (tx, rx) = mpsc::channel();
        let mut subscribers = self.subscribers.lock().ok()?;
        if subscribers.len() >= MAX_SUBSCRIBERS {
            return None;
        }
        subscribers.push(tx);
        Some((past, rx))
    }

    pub fn finish(&self, result: Value, csv: String, report: String, summary: String) {
        if let Ok(mut slot) = self.result.lock() {
            *slot = Some(result);
        }
        if let Ok(mut slot) = self.csv.lock() {
            *slot = Some(csv);
        }
        if let Ok(mut slot) = self.report.lock() {
            *slot = Some(report);
        }
        if let Ok(mut slot) = self.summary.lock() {
            *slot = Some(summary.clone());
        }
        self.emit("done", summary);
        if let Ok(mut status) = self.status.lock() {
            *status = RunStatus::Complete;
        }
    }

    pub fn fail(&self, message: impl Into<String>) {
        let message = message.into();
        if let Ok(mut status) = self.status.lock() {
            *status = RunStatus::Failed(message.clone());
        }
        self.emit("error", message);
    }

    pub fn is_terminal(&self) -> bool {
        !matches!(self.status(), RunStatus::Running)
    }
}

pub struct Store {
    runs: Mutex<HashMap<u64, Arc<RunRecord>>>,
    order: Mutex<VecDeque<u64>>,
    next: AtomicU64,
}

impl Store {
    pub fn new() -> Self {
        Self {
            runs: Mutex::new(HashMap::new()),
            order: Mutex::new(VecDeque::new()),
            next: AtomicU64::new(1),
        }
    }

    pub fn create(&self) -> Arc<RunRecord> {
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        let record = Arc::new(RunRecord::new(id));
        if let Ok(mut runs) = self.runs.lock() {
            runs.insert(id, record.clone());
        }
        if let Ok(mut order) = self.order.lock() {
            order.push_back(id);
            while order.len() > MAX_RUNS {
                if let Some(old) = order.pop_front() {
                    if let Ok(mut runs) = self.runs.lock() {
                        runs.remove(&old);
                    }
                }
            }
        }
        record
    }

    pub fn get(&self, id: u64) -> Option<Arc<RunRecord>> {
        self.runs.lock().ok()?.get(&id).cloned()
    }

    pub fn count(&self) -> usize {
        self.runs.lock().map(|r| r.len()).unwrap_or(0)
    }
}

impl Default for Store {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runs_are_bounded() {
        let store = Store::new();
        let first = store.create().id;
        for _ in 1..MAX_RUNS + 5 {
            store.create();
        }
        assert_eq!(store.count(), MAX_RUNS);
        assert!(store.get(first).is_none());
    }

    #[test]
    fn events_reach_subscribers() {
        let store = Store::new();
        let run = store.create();
        let (past, rx) = run.subscribe().unwrap();
        assert!(past.is_empty());
        run.emit("progress", r#"{"phase":"ascent"}"#);
        let event = rx.recv().unwrap();
        assert_eq!(event.name, "progress");
        assert!(event.data.contains("ascent"));
    }

    #[test]
    fn finish_sets_result_and_done() {
        let store = Store::new();
        let run = store.create();
        let (_past, rx) = run.subscribe().unwrap();
        run.finish(
            crate::json::object(vec![("ok", crate::json::Value::Bool(true))]),
            "csv".into(),
            "report".into(),
            "summary".into(),
        );
        assert_eq!(run.status(), RunStatus::Complete);
        assert!(run.result().is_some());
        let event = rx.recv().unwrap();
        assert_eq!(event.name, "done");
    }
}
