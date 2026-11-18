//! NeXosim owns model scheduling; simulated delays never use Tokio sleep.
use std::collections::VecDeque;
use std::time::Duration;

use anyhow::Result;
use nexosim::model::{Context, Model, schedulable};
use nexosim::ports::{EventSinkReader, EventSource, Output, SinkState, event_queue};
use nexosim::simulation::{Mailbox, SimInit};
use nexosim::time::MonotonicTime;
use serde::{Deserialize, Serialize};

use crate::{Admission, Config, Phase, Trace};

fn now<M: Model>(cx: &Context<M>) -> u64 {
    cx.time().duration_since(MonotonicTime::EPOCH).as_micros() as u64
}

#[derive(Clone, Serialize, Deserialize)]
struct Packet {
    id: u32,
    created_us: u64,
}

#[derive(Serialize, Deserialize)]
struct Readout {
    jobs: u32,
    period_us: u64,
    output: Output<Packet>,
    trace: Output<Trace>,
}

#[Model]
impl Readout {
    // The next readout is scheduled lazily: no up-front event per job.
    #[nexosim(schedulable)]
    async fn emit(&mut self, id: u32, cx: &Context<Self>) {
        let created_us = now(cx);
        self.trace
            .send(Trace::event(id, created_us, Phase::Readout))
            .await;
        self.output.send(Packet { id, created_us }).await;
        if id + 1 < self.jobs {
            cx.schedule_event(
                Duration::from_micros(self.period_us),
                schedulable!(Self::emit),
                id + 1,
            )
            .expect("validated positive period and bounded horizon");
        }
    }
}

#[derive(Serialize, Deserialize)]
struct Link {
    delay_us: u64,
    output: Output<Packet>,
    trace: Output<Trace>,
}

#[Model]
impl Link {
    fn receive(&mut self, packet: Packet, cx: &Context<Self>) {
        cx.schedule_event(
            Duration::from_micros(self.delay_us),
            schedulable!(Self::deliver),
            packet,
        )
        .expect("validated positive link delay and bounded horizon");
    }

    #[nexosim(schedulable)]
    async fn deliver(&mut self, packet: Packet, cx: &Context<Self>) {
        self.trace
            .send(Trace::event(packet.id, now(cx), Phase::LinkDelivered))
            .await;
        self.output.send(packet).await;
    }
}

#[derive(Serialize, Deserialize)]
struct Decoder {
    config: Config,
    // Owned mutable state: the model receives &mut self, so no Arc<Mutex<_>>.
    reservations: VecDeque<u64>,
    output: Output<Packet>,
    trace: Output<Trace>,
}

#[Model]
impl Decoder {
    async fn receive(&mut self, packet: Packet, cx: &Context<Self>) {
        let arrival = now(cx);
        // Half-open occupancy: a job ending at t does not occupy capacity at t.
        // This is independent of arrival/finish callback ordering at equal times.
        while self.reservations.front().is_some_and(|&t| t <= arrival) {
            self.reservations.pop_front();
        }
        if self.reservations.len() >= self.config.capacity {
            let mut record = Trace::event(packet.id, arrival, Phase::DecodeDropped);
            record.decoder_in_system = Some(self.reservations.len());
            self.trace.send(record).await;
            return;
        }
        let start = self
            .reservations
            .back()
            .copied()
            .unwrap_or(arrival)
            .max(arrival);
        let stalled =
            self.config.stall_every != 0 && (packet.id + 1).is_multiple_of(self.config.stall_every);
        let finish = start + self.config.decode_us + if stalled { self.config.stall_us } else { 0 };
        let estimated_finish = match self.config.admission {
            Admission::Fifo => None,
            Admission::DeadlineAware => Some(finish),
            Admission::DeadlineBlindToNewStall => Some(start + self.config.decode_us),
        };
        if estimated_finish.is_some_and(|t| {
            t + self.config.correction_us - packet.created_us > self.config.deadline_us
        }) {
            let mut record = Trace::event(packet.id, arrival, Phase::DeadlineRejected);
            record.decoder_in_system = Some(self.reservations.len());
            self.trace.send(record).await;
            return;
        }
        self.reservations.push_back(finish);
        let mut record = Trace::event(packet.id, arrival, Phase::DecodeAccepted);
        record.decoder_in_system = Some(self.reservations.len());
        record.queue_wait_us = Some(start - arrival);
        self.trace.send(record).await;
        cx.schedule_event(
            MonotonicTime::EPOCH + Duration::from_micros(finish),
            schedulable!(Self::finish),
            packet,
        )
        .expect("validated positive service time and bounded horizon");
    }

    #[nexosim(schedulable)]
    async fn finish(&mut self, packet: Packet, cx: &Context<Self>) {
        self.trace
            .send(Trace::event(packet.id, now(cx), Phase::DecodeCompleted))
            .await;
        self.output.send(packet).await;
    }
}

#[derive(Serialize, Deserialize)]
struct Controller {
    delay_us: u64,
    trace: Output<Trace>,
}

#[Model]
impl Controller {
    fn receive(&mut self, packet: Packet, cx: &Context<Self>) {
        cx.schedule_event(
            Duration::from_micros(self.delay_us),
            schedulable!(Self::apply),
            packet,
        )
        .expect("validated positive correction delay and bounded horizon");
    }

    #[nexosim(schedulable)]
    async fn apply(&mut self, packet: Packet, cx: &Context<Self>) {
        self.trace
            .send(Trace::event(packet.id, now(cx), Phase::CorrectionApplied))
            .await;
    }
}

pub fn simulate(c: &Config) -> Result<Vec<Trace>> {
    c.validate()?;
    let (sink, mut trace) = event_queue(SinkState::Enabled);
    let mut readout = Readout {
        jobs: c.jobs,
        period_us: c.period_us,
        output: Output::default(),
        trace: Output::default(),
    };
    let mut link = Link {
        delay_us: c.link_us,
        output: Output::default(),
        trace: Output::default(),
    };
    let mut decoder = Decoder {
        config: c.clone(),
        reservations: VecDeque::new(),
        output: Output::default(),
        trace: Output::default(),
    };
    let mut controller = Controller {
        delay_us: c.correction_us,
        trace: Output::default(),
    };
    let readout_box = Mailbox::new();
    let link_box = Mailbox::new();
    let decoder_box = Mailbox::new();
    let controller_box = Mailbox::new();
    readout.output.connect(Link::receive, &link_box);
    link.output.connect(Decoder::receive, &decoder_box);
    decoder.output.connect(Controller::receive, &controller_box);
    readout.trace.connect_sink(sink.clone());
    link.trace.connect_sink(sink.clone());
    decoder.trace.connect_sink(sink.clone());
    controller.trace.connect_sink(sink);

    let mut bench = SimInit::with_num_threads(c.simulation_threads);
    let input = EventSource::new()
        .connect(Readout::emit, &readout_box)
        .register(&mut bench);
    let mut simulation = bench
        .add_model(readout, readout_box, "readout")
        .add_model(link, link_box, "link")
        .add_model(decoder, decoder_box, "decoder")
        .add_model(controller, controller_box, "controller")
        .init(MonotonicTime::EPOCH)
        .map_err(|error| anyhow::anyhow!("NeXosim initialization failed: {error}"))?;
    // The scheduler requires a strictly future deadline. First job starts at 1 µs.
    simulation
        .scheduler()
        .schedule_event(Duration::from_micros(1), &input, 0)?;
    simulation
        .run()
        .map_err(|error| anyhow::anyhow!("NeXosim execution failed: {error}"))?;
    let mut records = Vec::new();
    while let Some(record) = trace.try_read() {
        records.push(record);
    }
    Ok(records)
}
