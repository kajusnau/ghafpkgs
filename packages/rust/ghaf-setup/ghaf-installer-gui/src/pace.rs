// SPDX-FileCopyrightText: 2026 TII (SSRC) and the Ghaf contributors
// SPDX-License-Identifier: Apache-2.0

//! Holds each step on screen long enough to be read.

use ghaf_setup_core::progress::{Phase, ProgressEvent, ProgressSender};
use std::time::Duration;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::time::Instant;

/// The shortest time a step shows as running.
pub const MIN_STEP: Duration = Duration::from_secs(1);

/// Forwards progress events in order, holding back each `PhaseFinished`
/// until its phase has shown as running for at least `min`. Returns once
/// `rx` closes and everything has been forwarded.
pub async fn pace(mut rx: UnboundedReceiver<ProgressEvent>, tx: ProgressSender, min: Duration) {
    let mut started: Vec<(Phase, Instant)> = Vec::new();
    while let Some(event) = rx.recv().await {
        match &event {
            ProgressEvent::PhaseStarted(phase) => started.push((*phase, Instant::now())),
            ProgressEvent::PhaseFinished(phase) => {
                if let Some((_, at)) = started.iter().find(|(p, _)| p == phase) {
                    tokio::time::sleep_until(*at + min).await;
                }
            }
            _ => {}
        }
        let _ = tx.send(event);
    }
}
