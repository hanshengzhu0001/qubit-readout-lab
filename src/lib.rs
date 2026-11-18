//! Synthetic classical message timing. No quantum states or physical QEC decoder.
pub mod model;
pub mod runner;

use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Admission {
    #[default]
    Fifo,
    DeadlineAware,
    DeadlineBlindToNewStall,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub jobs: u32,
    pub period_us: u64,
    pub link_us: u64,
    pub decode_us: u64,
    pub correction_us: u64,
    /// Capacity includes the job in service as well as waiting jobs.
    pub capacity: usize,
    pub deadline_us: u64,
    /// Every nth generated ID receives extra decode service, if admitted.
    pub stall_every: u32,
    pub stall_us: u64,
    pub payload_bytes: u64,
    pub simulation_threads: usize,
    pub admission: Admission,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            jobs: 500,
            period_us: 10,
            link_us: 2,
            decode_us: 4,
            correction_us: 1,
            capacity: 4,
            deadline_us: 12,
            stall_every: 0,
            stall_us: 20,
            payload_bytes: 32,
            simulation_threads: 1,
            admission: Admission::Fifo,
        }
    }
}

impl Config {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            (1..=100_000).contains(&self.jobs),
            "jobs must be 1..=100000"
        );
        ensure!(
            self.period_us > 0 && self.link_us > 0 && self.decode_us > 0 && self.correction_us > 0,
            "period, link, decode and correction must be positive"
        );
        ensure!(self.capacity > 0, "capacity must be positive");
        ensure!(
            (1..=2).contains(&self.simulation_threads),
            "simulation_threads must be 1 or 2"
        );
        let extra = if self.stall_every > 0 {
            self.stall_us
        } else {
            0
        };
        let horizon = 1u128
            + u128::from(self.jobs - 1) * u128::from(self.period_us)
            + u128::from(self.link_us)
            + u128::from(self.jobs) * (u128::from(self.decode_us) + u128::from(extra))
            + u128::from(self.correction_us);
        ensure!(
            horizon <= u128::from(u64::MAX),
            "simulated time would overflow u64 microseconds"
        );
        ensure!(
            u128::from(self.jobs) * u128::from(self.payload_bytes) <= u128::from(u64::MAX),
            "payload byte count would overflow"
        );
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    Readout,
    LinkDelivered,
    DecodeAccepted,
    DecodeCompleted,
    CorrectionApplied,
    DecodeDropped,
    DeadlineRejected,
}

impl Phase {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Readout => "readout",
            Self::LinkDelivered => "link_delivered",
            Self::DecodeAccepted => "decode_accepted",
            Self::DecodeCompleted => "decode_completed",
            Self::CorrectionApplied => "correction_applied",
            Self::DecodeDropped => "decode_dropped",
            Self::DeadlineRejected => "deadline_rejected",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Trace {
    pub id: u32,
    pub time_us: u64,
    pub phase: Phase,
    /// Set only on admission/drop; includes the job in service.
    pub decoder_in_system: Option<usize>,
    /// Set only on admission.
    pub queue_wait_us: Option<u64>,
}

impl Trace {
    pub fn event(id: u32, time_us: u64, phase: Phase) -> Self {
        Self {
            id,
            time_us,
            phase,
            decoder_in_system: None,
            queue_wait_us: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Metrics {
    pub generated: u32,
    pub completed: usize,
    pub capacity_drops: usize,
    pub deadline_rejections: usize,
    pub deadline_misses: usize,
    pub on_time: usize,
    pub max_decoder_in_system: usize,
    pub link_payload_bytes: u64,
    pub decoder_busy_us: u64,
    pub p50_latency_us: Option<u64>,
    pub p99_latency_us: Option<u64>,
    pub max_queue_wait_us: u64,
    pub simulated_until_us: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Report {
    pub name: String,
    pub config: Config,
    pub metrics: Metrics,
    pub trace: Vec<Trace>,
    /// Host time for bench setup, simulation, and trace collection only.
    pub simulation_wall_ns: u128,
    pub reference_checked: bool,
}

/// Independent FIFO arithmetic, with no simulator, actors, or model helpers.
/// The input order is fixed by a positive period and a constant link delay.
pub fn reference_trace(c: &Config) -> Result<Vec<Trace>> {
    c.validate()?;
    let mut completions: Vec<u64> = Vec::new();
    let mut trace = Vec::new();
    for id in 0..c.jobs {
        let created = 1 + u64::from(id) * c.period_us;
        let arrival = created + c.link_us;
        trace.push(Trace::event(id, created, Phase::Readout));
        trace.push(Trace::event(id, arrival, Phase::LinkDelivered));
        // A reservation ending exactly at arrival has already freed capacity.
        let active = completions.len() - completions.partition_point(|&t| t <= arrival);
        if active >= c.capacity {
            let mut drop = Trace::event(id, arrival, Phase::DecodeDropped);
            drop.decoder_in_system = Some(active);
            trace.push(drop);
            continue;
        }
        let start = arrival.max(completions.last().copied().unwrap_or(0));
        let stalled = c.stall_every != 0 && (id + 1).is_multiple_of(c.stall_every);
        let finished = start + c.decode_us + if stalled { c.stall_us } else { 0 };
        let estimate = match c.admission {
            Admission::Fifo => None,
            Admission::DeadlineAware => Some(finished),
            Admission::DeadlineBlindToNewStall => Some(start + c.decode_us),
        };
        if estimate.is_some_and(|t| t + c.correction_us - created > c.deadline_us) {
            let mut rejection = Trace::event(id, arrival, Phase::DeadlineRejected);
            rejection.decoder_in_system = Some(active);
            trace.push(rejection);
            continue;
        }
        completions.push(finished);
        let mut accepted = Trace::event(id, arrival, Phase::DecodeAccepted);
        accepted.decoder_in_system = Some(active + 1);
        accepted.queue_wait_us = Some(start - arrival);
        trace.push(accepted);
        trace.push(Trace::event(id, finished, Phase::DecodeCompleted));
        trace.push(Trace::event(
            id,
            finished + c.correction_us,
            Phase::CorrectionApplied,
        ));
    }
    canonicalize(&mut trace);
    Ok(trace)
}

/// A comparison order, not a claim of global callback order across actors.
pub fn canonicalize(trace: &mut [Trace]) {
    trace.sort_unstable_by_key(|t| (t.time_us, t.id, t.phase));
}

pub fn check_reference(config: &Config, trace: &[Trace]) -> Result<()> {
    let expected = reference_trace(config)?;
    ensure!(
        trace.len() == expected.len(),
        "trace length differs from reference: {} vs {}",
        trace.len(),
        expected.len()
    );
    for (actual, expected) in trace.iter().zip(&expected) {
        ensure!(
            actual == expected,
            "reference mismatch: actual {actual:?}, expected {expected:?}"
        );
    }
    Ok(())
}

pub fn summarize(c: &Config, trace: &[Trace]) -> Metrics {
    let mut latency: Vec<_> = trace
        .iter()
        .filter(|t| t.phase == Phase::CorrectionApplied)
        .map(|t| t.time_us - (1 + u64::from(t.id) * c.period_us))
        .collect();
    latency.sort_unstable();
    let deadline_misses = latency.iter().filter(|&&t| t > c.deadline_us).count();
    let decoder_busy_us = trace
        .iter()
        .filter(|t| t.phase == Phase::DecodeCompleted)
        .map(|t| {
            let stalls = c.stall_every != 0 && (t.id + 1).is_multiple_of(c.stall_every);
            c.decode_us + if stalls { c.stall_us } else { 0 }
        })
        .sum();
    let quantile = |percent: usize| -> Option<u64> {
        if latency.is_empty() {
            None
        } else {
            Some(latency[(percent * latency.len()).div_ceil(100) - 1])
        }
    };
    Metrics {
        generated: c.jobs,
        completed: latency.len(),
        capacity_drops: trace
            .iter()
            .filter(|t| t.phase == Phase::DecodeDropped)
            .count(),
        deadline_rejections: trace
            .iter()
            .filter(|t| t.phase == Phase::DeadlineRejected)
            .count(),
        deadline_misses,
        on_time: latency.len() - deadline_misses,
        max_decoder_in_system: trace
            .iter()
            .filter_map(|t| t.decoder_in_system)
            .max()
            .unwrap_or(0),
        link_payload_bytes: u64::from(c.jobs) * c.payload_bytes,
        decoder_busy_us,
        p50_latency_us: quantile(50),
        p99_latency_us: quantile(99),
        max_queue_wait_us: trace
            .iter()
            .filter_map(|t| t.queue_wait_us)
            .max()
            .unwrap_or(0),
        simulated_until_us: trace.last().map_or(0, |t| t.time_us),
    }
}

/// Execute NeXosim, then independently verify every record before reporting success.
pub fn run(name: String, config: Config) -> Result<Report> {
    config.validate()?;
    let started = std::time::Instant::now();
    let mut trace = model::simulate(&config)?;
    let simulation_wall_ns = started.elapsed().as_nanos();
    canonicalize(&mut trace);
    check_reference(&config, &trace)?;
    let metrics = summarize(&config, &trace);
    ensure!(
        metrics.completed + metrics.capacity_drops + metrics.deadline_rejections
            == config.jobs as usize,
        "conservation violated"
    );
    ensure!(
        metrics.max_decoder_in_system <= config.capacity,
        "decoder capacity exceeded"
    );
    Ok(Report {
        name,
        config,
        metrics,
        trace,
        simulation_wall_ns,
        reference_checked: true,
    })
}
