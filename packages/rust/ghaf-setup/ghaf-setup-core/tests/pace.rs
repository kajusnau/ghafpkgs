// SPDX-FileCopyrightText: 2026 TII (SSRC) and the Ghaf contributors
// SPDX-License-Identifier: Apache-2.0

use ghaf_setup_core::pace::{Update, pace, run};
use ghaf_setup_core::progress::{Phase, ProgressEvent};
use std::time::Duration;
use tokio::sync::mpsc::{UnboundedReceiver, unbounded_channel};
use tokio::time::Instant;

const MIN: Duration = Duration::from_secs(1);

fn drain(rx: &mut UnboundedReceiver<Update>) -> Vec<String> {
    let mut updates = Vec::new();
    while let Ok(update) = rx.try_recv() {
        updates.push(format!("{update:?}"));
    }
    updates
}

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

    assert_eq!(
        drain(&mut out_rx),
        [
            "Progress(PhaseStarted(Wipe))",
            "Progress(PhaseFinished(Wipe))",
            "Progress(PhaseStarted(WriteImage))"
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

#[tokio::test(start_paused = true)]
async fn the_result_comes_after_every_step_it_ends() {
    let (out_tx, mut out_rx) = unbounded_channel();

    run(
        |tx| async move {
            tx.send(ProgressEvent::PhaseStarted(Phase::SecureBoot))
                .unwrap();
            tx.send(ProgressEvent::Failed {
                phase: Phase::SecureBoot,
                message: "no keys".into(),
                recoverable: true,
            })
            .unwrap();
            Err("no keys".to_string())
        },
        out_tx,
        MIN,
    )
    .await;

    let updates = drain(&mut out_rx);
    assert_eq!(updates.len(), 3);
    assert!(updates[1].starts_with("Progress(Failed"));
    assert_eq!(updates[2], r#"Finished(Err("no keys"))"#);
}
