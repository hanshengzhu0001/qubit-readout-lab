//! Sequential state assignment in NeXosim, with Tokio coordinating independent benches.
use anyhow::{Result, anyhow, ensure};
use nexosim::model::{Context, Model, schedulable};
use nexosim::ports::{EventSinkReader, EventSource, Output, SinkState, event_queue};
use nexosim::simulation::{Mailbox, SimInit};
use nexosim::time::MonotonicTime;
use serde::{Deserialize, Serialize};
use std::{sync::Arc, time::Duration};
use tokio::{sync::Semaphore, task::JoinSet};

const WINDOW: usize = 48;
const TICK_NS: u64 = 4;
const FEEDBACK_NS: u64 = 16;
type Iq = [f64; 2];

#[derive(Deserialize)]
struct Shot {
    state: bool,
    samples: Vec<Iq>,
}

#[derive(Deserialize)]
struct Input {
    templates: [Vec<Iq>; 2],
    sigma: f64,
    nominal: Vec<Shot>,
    drift: Vec<Shot>,
    pilots: Vec<Shot>,
}

#[derive(Clone, Serialize, Deserialize)]
struct Sample {
    index: usize,
    contributions: Vec<f64>,
}

#[derive(Clone, Serialize, Deserialize)]
struct Decision {
    samples: usize,
    state: bool,
    feedback_ns: u64,
    evidence: Vec<f64>,
}

#[derive(Serialize, Deserialize)]
struct Reader {
    output: Output<Decision>,
    threshold: f64,
    evidence: Vec<f64>,
}

fn logical_ns(time: MonotonicTime) -> u64 {
    time.duration_since(MonotonicTime::EPOCH).as_nanos() as u64
}

#[Model]
impl Reader {
    fn begin(&mut self, contributions: Vec<f64>, cx: &Context<Self>) {
        self.evidence.clear();
        cx.schedule_event(
            Duration::from_nanos(TICK_NS),
            schedulable!(Self::sample),
            Sample {
                index: 0,
                contributions,
            },
        )
        .unwrap();
    }

    #[nexosim(schedulable)]
    fn sample(&mut self, mut sample: Sample, cx: &Context<Self>) {
        let odds =
            self.evidence.last().copied().unwrap_or(0.0) + sample.contributions[sample.index];
        self.evidence.push(odds);
        sample.index += 1;
        if odds.abs() >= self.threshold || sample.index == WINDOW {
            cx.schedule_event(
                Duration::from_nanos(FEEDBACK_NS),
                schedulable!(Self::emit),
                (),
            )
            .unwrap();
        } else {
            cx.schedule_event(
                Duration::from_nanos(TICK_NS),
                schedulable!(Self::sample),
                sample,
            )
            .unwrap();
        }
    }

    #[nexosim(schedulable)]
    async fn emit(&mut self, _: (), cx: &Context<Self>) {
        self.output
            .send(Decision {
                samples: self.evidence.len(),
                state: self.evidence.last().copied().unwrap_or(0.0) > 0.0,
                feedback_ns: logical_ns(cx.time()),
                evidence: std::mem::take(&mut self.evidence),
            })
            .await;
    }
}

fn fit_phase(input: &Input) -> f64 {
    let (mut dot, mut cross) = (0., 0.);
    for shot in &input.pilots {
        for (&[a, b], &[i, q]) in input.templates[usize::from(shot.state)]
            .iter()
            .zip(&shot.samples)
        {
            dot += a * i + b * q;
            cross += a * q - b * i;
        }
    }
    cross.atan2(dot)
}

#[derive(Serialize)]
struct Preview<'a> {
    samples: &'a [Iq],
    truth: bool,
    decision: bool,
    evidence: Vec<f64>,
}

fn evaluate(input: &Input, mode: &str, threshold: f64) -> Result<serde_json::Value> {
    let angle = if mode == "corrected" {
        fit_phase(input)
    } else {
        0.
    };
    let (sin, cos) = angle.sin_cos();
    let shots = if mode == "nominal" {
        &input.nominal
    } else {
        &input.drift
    };
    let (writer, mut reader) = event_queue(SinkState::Enabled);
    let mut actor = Reader {
        output: Output::default(),
        threshold,
        evidence: Vec::new(),
    };
    actor.output.connect_sink(writer);
    let mailbox = Mailbox::new();
    let mut bench = SimInit::with_num_threads(1);
    let source = EventSource::new()
        .connect(Reader::begin, &mailbox)
        .register(&mut bench);
    let mut sim = bench
        .add_model(actor, mailbox, "readout")
        .init(MonotonicTime::EPOCH)
        .map_err(|e| anyhow!("{e}"))?;
    let mut previews = Vec::new();
    let (mut errors, mut samples) = (0, 0);
    let mut preview_counts = [0; 2];
    for shot in shots {
        let scores: Vec<_> = shot
            .samples
            .iter()
            .enumerate()
            .map(|(t, &[i, q])| {
                let (i, q) = (cos * i + sin * q, cos * q - sin * i);
                let residual = |[a, b]: Iq| (i - a).powi(2) + (q - b).powi(2);
                (residual(input.templates[0][t]) - residual(input.templates[1][t]))
                    / (2. * input.sigma.powi(2))
            })
            .collect();
        let mut odds = 0.;
        let expected = scores
            .iter()
            .position(|x| {
                odds += x;
                odds.abs() >= threshold
            })
            .map_or(WINDOW, |i| i + 1);
        let start = logical_ns(sim.time());
        sim.process_event(&source, scores)
            .map_err(|e| anyhow!("{e}"))?;
        sim.run().map_err(|e| anyhow!("{e}"))?;
        let decision = reader
            .try_read()
            .ok_or_else(|| anyhow!("missing feedback"))?;
        ensure!(
            decision.samples == expected
                && decision.state == (odds > 0.)
                && decision.feedback_ns == start + expected as u64 * TICK_NS + FEEDBACK_NS
                && reader.try_read().is_none(),
            "feedback differs from direct calculation"
        );
        errors += usize::from(decision.state != shot.state);
        samples += decision.samples;
        let state_index = usize::from(shot.state);
        if preview_counts[state_index] < 48 {
            preview_counts[state_index] += 1;
            previews.push(Preview {
                samples: &shot.samples,
                truth: shot.state,
                decision: decision.state,
                evidence: decision.evidence,
            });
        }
    }
    Ok(
        serde_json::json!({"mode":mode,"threshold":threshold,"phase_deg":angle.to_degrees(),
        "errors":errors,"shots":shots.len(),"mean_samples":samples as f64/shots.len() as f64,
        "previews":previews}),
    )
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let path = std::env::args()
        .nth(1)
        .ok_or_else(|| anyhow!("supply fixture JSON"))?;
    let input: Arc<Input> = Arc::new(serde_json::from_reader(std::fs::File::open(path)?)?);
    ensure!(
        input.sigma.is_finite()
            && input.sigma > 0.
            && input.templates.iter().all(|t| t.len() == WINDOW)
            && [&input.nominal, &input.drift, &input.pilots]
                .iter()
                .all(|shots| !shots.is_empty() && shots.iter().all(|s| s.samples.len() == WINDOW)),
        "invalid fixture dimensions or noise scale"
    );
    let permits = Arc::new(Semaphore::new(2));
    let mut tasks = JoinSet::new();
    for mode in ["nominal", "stale", "corrected"] {
        for threshold in [5., f64::INFINITY] {
            let input = Arc::clone(&input);
            let permit = Arc::clone(&permits).acquire_owned().await?;
            tasks.spawn_blocking(move || {
                let _permit = permit;
                evaluate(&input, mode, threshold)
            });
        }
    }
    let mut results = Vec::new();
    while let Some(result) = tasks.join_next().await {
        results.push(result??);
    }
    results.sort_by_key(|r| {
        (
            r["mode"].as_str().unwrap().to_owned(),
            r["threshold"].is_null(),
        )
    });
    println!(
        "{}",
        serde_json::to_string(&serde_json::json!({"templates":input.templates,"runs":results}))?
    );
    Ok(())
}
