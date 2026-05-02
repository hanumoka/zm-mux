//! Throttled / dedup'd OSC-driven desktop notification dispatcher.
//!
//! Wraps `notify-rust` with two backpressure layers:
//!
//! 1. **Token bucket throttle** — refills at `TOKEN_RATE` tokens/sec,
//!    capacity `TOKEN_BURST`.  Drops notifications when empty so a noisy
//!    pane (e.g. a build script chattering with OSC 9) cannot spam the OS
//!    notification center.
//! 2. **Dedup window** — `(title, body)` pairs seen within `DEDUP_WINDOW`
//!    are coalesced to one.  Defends against agents that re-emit the same
//!    "ready" signal while polling.
//!
//! Currently global (one bucket / one dedup table for the whole app).
//! Per-pane buckets are a follow-up — global is sufficient for MVP and
//! avoids touching the dispatch sites every time a pane is spawned.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use zm_term::{OscEvent, OscEventKind};

const TOKEN_RATE: f32 = 2.0;
const TOKEN_BURST: f32 = 5.0;
const DEDUP_WINDOW: Duration = Duration::from_secs(5);
const NOTIFICATION_TIMEOUT_MS: u32 = 5000;

pub struct NotifyDispatcher {
    last_seen: HashMap<String, Instant>,
    bucket_tokens: f32,
    bucket_last_refill: Instant,
}

impl NotifyDispatcher {
    pub fn new() -> Self {
        Self {
            last_seen: HashMap::new(),
            bucket_tokens: TOKEN_BURST,
            bucket_last_refill: Instant::now(),
        }
    }

    pub fn dispatch(&mut self, event: &OscEvent) {
        let (title, body) = match &event.kind {
            OscEventKind::Notify { title, body } => (title.as_str(), body.as_str()),
        };
        let dedup_key = format!("{title}\u{0}{body}");
        let now = Instant::now();

        // Refill tokens against wall clock since last call.
        let elapsed = now.duration_since(self.bucket_last_refill).as_secs_f32();
        self.bucket_tokens = (self.bucket_tokens + elapsed * TOKEN_RATE).min(TOKEN_BURST);
        self.bucket_last_refill = now;

        if let Some(last) = self.last_seen.get(&dedup_key)
            && now.duration_since(*last) < DEDUP_WINDOW
        {
            return;
        }
        if self.bucket_tokens < 1.0 {
            return;
        }
        self.bucket_tokens -= 1.0;
        self.last_seen.insert(dedup_key, now);

        // Best-effort dispatch.  No notification daemon (headless Linux,
        // disabled toast on Win) → silent drop.  Never panic the renderer.
        let _ = notify_rust::Notification::new()
            .summary(title)
            .body(body)
            .timeout(notify_rust::Timeout::Milliseconds(NOTIFICATION_TIMEOUT_MS))
            .show();
    }

    /// Drop dedup entries older than `2 * DEDUP_WINDOW`.  Cheap; OK to call
    /// every frame.  Without this the table grows unboundedly for distinct
    /// notification bodies.
    pub fn gc(&mut self) {
        let cutoff = DEDUP_WINDOW * 2;
        let now = Instant::now();
        self.last_seen
            .retain(|_, t| now.duration_since(*t) < cutoff);
    }
}

impl Default for NotifyDispatcher {
    fn default() -> Self {
        Self::new()
    }
}
