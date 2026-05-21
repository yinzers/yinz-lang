//! `$/progress` notification helpers for long-running LSP operations.
//!
//! Progress notifications inform the client to show a progress indicator
//! (e.g. a spinner in the status bar) for operations like find-references
//! or rename that may scan many files.

use lsp_server::{Message, Notification};
use lsp_types::{
    NumberOrString, ProgressParams, ProgressParamsValue, WorkDoneProgress,
    WorkDoneProgressBegin, WorkDoneProgressEnd, WorkDoneProgressReport,
};
use std::sync::atomic::{AtomicU32, Ordering};

/// Monotonically increasing token source — unique per request within a session.
static NEXT_TOKEN: AtomicU32 = AtomicU32::new(1);

/// A paired begin/end progress tracker for one LSP operation.
///
/// Call `begin` to start a progress indicator on the client; call `end` to
/// dismiss it. The `begin`/`end` pair MUST be matched — always call `end`
/// after `begin`, even if the operation fails.
pub struct ProgressTracker {
    token: NumberOrString,
    sender: crossbeam_channel::Sender<Message>,
}

impl ProgressTracker {
    /// Emit a `$/progress` `begin` notification and return a tracker.
    ///
    /// `title` is the human-readable operation name shown in the client's
    /// status bar (e.g. "Finding references").
    pub fn begin(title: &str, sender: crossbeam_channel::Sender<Message>) -> Self {
        let id = NEXT_TOKEN.fetch_add(1, Ordering::Relaxed);
        // Use a String token to avoid u32→i32 narrowing (LSP NumberOrString::Number is i32,
        // which would wrap after 2^31 begins in a long-lived session).
        let token = NumberOrString::String(format!("ynz-{id}"));

        let notif = Notification {
            method: "$/progress".to_string(),
            params: serde_json::to_value(ProgressParams {
                token: token.clone(),
                value: ProgressParamsValue::WorkDone(WorkDoneProgress::Begin(
                    WorkDoneProgressBegin {
                        title: title.to_string(),
                        cancellable: Some(false),
                        message: None,
                        percentage: None,
                    },
                )),
            })
            .unwrap_or_default(),
        };
        sender.send(Message::Notification(notif)).ok();

        Self { token, sender }
    }

    /// Emit a `$/progress` `report` notification (optional mid-scan update).
    ///
    /// `percentage`: 0-100 if the caller can compute progress, `None` if indeterminate.
    /// `message`: short status string visible in the client status bar.
    pub fn report(&self, percentage: Option<u32>, message: Option<&str>) {
        let notif = Notification {
            method: "$/progress".to_string(),
            params: serde_json::to_value(ProgressParams {
                token: self.token.clone(),
                value: ProgressParamsValue::WorkDone(WorkDoneProgress::Report(
                    WorkDoneProgressReport {
                        cancellable: Some(false),
                        message: message.map(str::to_string),
                        percentage: percentage.map(|p| p.min(100)),
                    },
                )),
            })
            .unwrap_or_default(),
        };
        self.sender.send(Message::Notification(notif)).ok();
    }

    /// Emit a `$/progress` `end` notification to dismiss the progress indicator.
    pub fn end(self, message: Option<&str>) {
        let notif = Notification {
            method: "$/progress".to_string(),
            params: serde_json::to_value(ProgressParams {
                token: self.token,
                value: ProgressParamsValue::WorkDone(WorkDoneProgress::End(WorkDoneProgressEnd {
                    message: message.map(str::to_string),
                })),
            })
            .unwrap_or_default(),
        };
        self.sender.send(Message::Notification(notif)).ok();
    }
}
