use nexosim_control_lab::runner::{run_cases, sweep_cases};
use nexosim_control_lab::{Admission, Config, Phase, run};

#[test]
fn arrival_at_service_completion_can_reuse_the_only_slot() {
    let config = Config {
        jobs: 3,
        period_us: 4,
        capacity: 1,
        deadline_us: 7,
        admission: Admission::DeadlineAware,
        simulation_threads: 2,
        ..Config::default()
    };
    let report = run("boundary".into(), config).unwrap();
    assert_eq!(report.metrics.on_time, 3);
    assert_eq!(report.metrics.max_decoder_in_system, 1);
    let correction_times: Vec<_> = report
        .trace
        .iter()
        .filter(|event| event.phase == Phase::CorrectionApplied)
        .map(|event| event.time_us)
        .collect();
    assert_eq!(correction_times, [8, 12, 16]);
}

#[test]
fn rejecting_a_known_stall_saves_a_later_message() {
    let config = Config {
        jobs: 3,
        period_us: 5,
        capacity: 2,
        stall_every: 2,
        ..Config::default()
    };
    let fifo = run("fifo".into(), config.clone()).unwrap().metrics;
    let blind = run(
        "blind".into(),
        Config {
            admission: Admission::DeadlineBlindToNewStall,
            ..config.clone()
        },
    )
    .unwrap()
    .metrics;
    let aware = run(
        "aware".into(),
        Config {
            admission: Admission::DeadlineAware,
            ..config
        },
    )
    .unwrap()
    .metrics;
    assert_eq!(
        (fifo.on_time, fifo.deadline_misses, fifo.decoder_busy_us),
        (1, 2, 32)
    );
    assert_eq!(
        (
            blind.on_time,
            blind.deadline_misses,
            blind.deadline_rejections,
            blind.decoder_busy_us
        ),
        (1, 1, 1, 28)
    );
    assert_eq!(
        (
            aware.on_time,
            aware.deadline_rejections,
            aware.decoder_busy_us
        ),
        (2, 1, 8)
    );
}

#[tokio::test]
async fn changing_sweep_concurrency_preserves_each_trace() {
    let serial = run_cases(sweep_cases(24), 1).await.unwrap();
    let parallel = run_cases(sweep_cases(24), 2).await.unwrap();
    assert_eq!(serial.len(), 84);
    assert_eq!(serial.len(), parallel.len());
    for (a, b) in serial.iter().zip(&parallel) {
        assert_eq!(a.name, b.name);
        assert_eq!(a.metrics, b.metrics);
        assert_eq!(a.trace, b.trace);
    }
}
