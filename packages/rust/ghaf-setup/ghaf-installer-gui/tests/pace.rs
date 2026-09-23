// SPDX-FileCopyrightText: 2026 TII (SSRC) and the Ghaf contributors
// SPDX-License-Identifier: Apache-2.0

use ghaf_installer_gui::pace::pace;
use ghaf_setup_core::progress::{Phase, ProgressEvent};
use std::time::Duration;
use tokio::sync::mpsc::unbounded_channel;
use tokio::time::Instant;

const MIN: Duration = Duration::from_secs(1);

#[tokio::test(start_paused = true)]
async fn a_quick_step_is_held_for_the_minimum() {
    let (in_tx, in_rx) = unbounded_channel();
    let (out_tx, mut out_rx) = unbounded_channel();
    in_tx
        .send(ProgressEvent::PhaseStarted(Phase::Wipe))
        .unwrap();
    in_tx
        .send(ProgressEvent::PhaseFinished(Phase::Wipe))
        .unwrap();
    in_tx
        .send(ProgressEvent::PhaseStarted(Phase::WriteImage))
        .unwrap();
    drop(in_tx);

    let start = Instant::now();
    pace(in_rx, out_tx, MIN).await;
    assert_eq!(start.elapsed(), MIN);

    let mut events = Vec::new();
    while let Ok(event) = out_rx.try_recv() {
        events.push(format!("{event:?}"));
    }
    assert_eq!(
        events,
        [
            "PhaseStarted(Wipe)",
            "PhaseFinished(Wipe)",
            "PhaseStarted(WriteImage)"
        ],
        "order is kept"
    );
}

#[tokio::test(start_paused = true)]
async fn a_slow_step_is_not_held_further() {
    let (in_tx, in_rx) = unbounded_channel();
    let (out_tx, _out_rx) = unbounded_channel();
    let feeder = tokio::spawn(async move {
        in_tx
            .send(ProgressEvent::PhaseStarted(Phase::Wipe))
            .unwrap();
        tokio::time::sleep(Duration::from_secs(3)).await;
        in_tx
            .send(ProgressEvent::PhaseFinished(Phase::Wipe))
            .unwrap();
    });

    let start = Instant::now();
    pace(in_rx, out_tx, MIN).await;
    feeder.await.unwrap();
    assert_eq!(start.elapsed(), Duration::from_secs(3));
}
