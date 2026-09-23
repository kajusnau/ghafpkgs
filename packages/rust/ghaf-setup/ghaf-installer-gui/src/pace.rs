// SPDX-FileCopyrightText: 2026 TII (SSRC) and the Ghaf contributors
// SPDX-License-Identifier: Apache-2.0

//! Runs an install or erase, and holds each step on screen long enough to
//! be read.

use ghaf_setup_core::progress::{Phase, ProgressEvent, ProgressSender};
use std::future::Future;
use std::time::Duration;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};
use tokio::time::Instant;

/// The shortest time a step shows as running.
pub const MIN_STEP: Duration = Duration::from_secs(1);

/// What the running page receives, in order.
#[derive(Debug, Clone)]
pub enum Update {
    Progress(ProgressEvent),
    /// Always the last update of a run.
    Finished(Result<(), String>),
}

/// Runs `work` with a progress sender, forwarding its progress paced and then
/// its result. The result comes through the same channel, after every
/// progress event, so the page never sees an outcome before the steps that
/// led to it.
pub async fn run<F, Fut>(work: F, out: UnboundedSender<Update>, min: Duration)
where
    F: FnOnce(ProgressSender) -> Fut,
    Fut: Future<Output = Result<(), String>>,
{
    let (tx, rx) = unbounded_channel();
    let pacing = tokio::spawn(pace(rx, out.clone(), min));
    // `work` owns the only sender, so the pacer ends once it returns.
    let result = work(tx).await;
    let _ = pacing.await;
    let _ = out.send(Update::Finished(result));
}

/// Forwards progress events in order, holding back each `PhaseFinished`
/// until its phase has shown as running for at least `min`. Returns once
/// `rx` closes and everything has been forwarded.
pub async fn pace(
    mut rx: UnboundedReceiver<ProgressEvent>,
    out: UnboundedSender<Update>,
    min: Duration,
) {
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
        let _ = out.send(Update::Progress(event));
    }
}
