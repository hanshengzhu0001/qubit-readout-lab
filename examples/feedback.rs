// Replay sequential readout decisions on a logical nanosecond clock.
use anyhow::{Result, ensure};
use nexosim::model::{Context, Model, schedulable};
use nexosim::ports::{EventSinkReader, EventSource, Output, SinkState, event_queue};
use nexosim::simulation::{Mailbox, SimInit};
use nexosim::time::MonotonicTime;
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Default, Serialize, Deserialize)]
struct Feedback {
    out: Output<u64>,
}

#[Model]
impl Feedback {
    fn start(&mut self, samples: u64, cx: &Context<Self>) {
        cx.schedule_event(
            Duration::from_nanos(4 * samples + 16),
            schedulable!(Self::finish),
            (),
        )
        .unwrap();
    }
    #[nexosim(schedulable)]
    async fn finish(&mut self, _event: (), cx: &Context<Self>) {
        self.out
            .send(cx.time().duration_since(MonotonicTime::EPOCH).as_nanos() as u64)
            .await;
    }
}

fn main() -> Result<()> {
    let path = std::env::args()
        .nth(1)
        .ok_or_else(|| anyhow::anyhow!("expected sample-count JSON"))?;
    let counts: Vec<u64> = serde_json::from_reader(std::fs::File::open(path)?)?;
    ensure!(
        !counts.is_empty() && counts.iter().all(|n| (1..=48).contains(n)),
        "expected counts from 1 to 48"
    );
    let mut bench = SimInit::with_num_threads(1);
    let mailbox = Mailbox::new();
    let input = EventSource::new()
        .connect(Feedback::start, &mailbox)
        .register(&mut bench);
    let (writer, mut reader) = event_queue(SinkState::Enabled);
    let mut actor = Feedback::default();
    actor.out.connect_sink(writer);
    let mut sim = bench
        .add_model(actor, mailbox, "feedback")
        .init(MonotonicTime::EPOCH)
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    for n in &counts {
        let start = sim.time();
        sim.process_event(&input, *n)
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        sim.step().map_err(|e| anyhow::anyhow!("{e}"))?;
        let expected = start.duration_since(MonotonicTime::EPOCH).as_nanos() as u64 + 4 * n + 16;
        ensure!(
            reader.try_read() == Some(expected),
            "unexpected feedback timestamp"
        );
    }
    println!("Verified {} serial feedback events", counts.len());
    Ok(())
}
