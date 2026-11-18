//! Tokio coordinates independent experiments; each simulation has its own executor.
use crate::{Admission, Config, Report, run};
use anyhow::{Context, Result, ensure};
use std::sync::Arc;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;

/// Acquire before spawn: only `parallelism` blocking simulations can be active.
/// Completed outputs and task handles are retained for this small fixed sweep.
pub async fn run_cases(cases: Vec<(String, Config)>, parallelism: usize) -> Result<Vec<Report>> {
    ensure!((1..=2).contains(&parallelism), "parallelism must be 1 or 2");
    for (_, config) in &cases {
        config.validate()?;
    }
    let permits = Arc::new(Semaphore::new(parallelism));
    let mut tasks = JoinSet::new();
    for (name, config) in cases {
        let permit = Arc::clone(&permits).acquire_owned().await?;
        tasks.spawn_blocking(move || {
            // Permit lives until simulation finishes, including error/panic unwinding.
            let _permit = permit;
            run(name, config)
        });
    }
    let mut reports = Vec::new();
    let mut errors = Vec::new();
    // Drain all jobs: already-started spawn_blocking work cannot be aborted.
    while let Some(result) = tasks.join_next().await {
        match result
            .context("simulation task panicked or was cancelled")
            .and_then(|r| r)
        {
            Ok(report) => reports.push(report),
            Err(error) => errors.push(format!("{error:#}")),
        }
    }
    ensure!(
        errors.is_empty(),
        "experiment sweep failed: {}",
        errors.join("; ")
    );
    reports.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(reports)
}

pub fn sweep_cases(jobs: u32) -> Vec<(String, Config)> {
    let mut cases = Vec::new();
    for period in [2, 3, 4, 5, 6, 8, 10] {
        for capacity in [2, 8] {
            for stalled in [false, true] {
                for (policy, admission) in [
                    ("fifo", Admission::Fifo),
                    ("deadline", Admission::DeadlineAware),
                    ("blind", Admission::DeadlineBlindToNewStall),
                ] {
                    let config = Config {
                        jobs,
                        period_us: period,
                        capacity,
                        admission,
                        stall_every: if stalled { 10 } else { 0 },
                        ..Config::default()
                    };
                    let load = if stalled { "stall" } else { "normal" };
                    cases.push((
                        format!("period{period}_capacity{capacity}_{load}_{policy}"),
                        config,
                    ));
                }
            }
        }
    }
    cases
}
