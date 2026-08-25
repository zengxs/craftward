// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

use std::ffi::c_void;
use std::path::PathBuf;
use std::sync::Mutex;

use prost::Message as _;
use ward_codex::{AgentMessagePhase, Thread, ThreadItem, ThreadSummary, Turn, TurnStatus};

use super::super::{WardBuffer, wire};
use super::events::HistoryEventSink;

#[derive(Default)]
pub(super) struct CapturedEvent {
    pub(super) events: Vec<wire::HistoryEvent>,
}

unsafe extern "C" fn capture_event(context: *mut c_void, event: *const WardBuffer) {
    // SAFETY: This callback is used only with the live mutex and buffer
    // supplied by `HistoryEventSink::emit` below.
    let captured = unsafe { &*(context.cast::<Mutex<CapturedEvent>>()) };
    // SAFETY: The event buffer is valid for this callback.
    let event = unsafe { &*event };
    let event = wire::HistoryEvent::decode(event.bytes.as_ref()).unwrap();
    captured.lock().unwrap().events.push(event);
}

pub(super) fn event_sink(captured: &Mutex<CapturedEvent>) -> HistoryEventSink {
    HistoryEventSink::new(
        capture_event,
        std::ptr::from_ref(captured).cast_mut().cast(),
    )
}

pub(super) fn thread() -> Thread {
    Thread {
        summary: ThreadSummary {
            id: "thread-1".to_owned(),
            name: Some("Example".to_owned()),
            preview: "Preview".to_owned(),
            cwd: PathBuf::from("/workspace"),
            created_at_unix_seconds: 10,
            updated_at_unix_seconds: 20,
        },
        turns: vec![Turn {
            id: "turn-1".to_owned(),
            status: TurnStatus::Completed,
            timing: Default::default(),
            items: vec![ThreadItem::AgentMessage {
                id: "agent-1".to_owned(),
                text: "Done".to_owned(),
                phase: Some(AgentMessagePhase::FinalAnswer),
            }],
        }],
    }
}
