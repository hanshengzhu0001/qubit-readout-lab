# Qubit Readout Lab

**Should a quantum computer flip a bit or leave it alone?** To get a bit ready for its next job, the machine needs to leave it at 0. If it’s already 0, do nothing. If it’s 1, flip it. Either action can be a mistake.

The tricky part is knowing which state it’s in. In this experiment, two quantum bits (qubits) are prepared so their measurements agree. The machine measures one to decide what to do to the other. Its sensor returns a noisy microwave signal. If that signal drifts, yesterday’s calibration can make a 1 look like a 0.

**Watch one missed reset.** The old calibration tells the machine to leave the qubit alone. It finishes before the deadline, with the qubit still at 1. Replay the same signal with a freshly calibrated sensor and the machine recognizes the 1 and flips the qubit to 0.

[Replay the shot](https://hanshengzhu0001.github.io/qubit-readout-lab/) · [Follow the circuit](https://hanshengzhu0001.github.io/qubit-readout-lab/machine.html) · [Read the Rust implementation](examples/readout.rs)

![Decision evidence for one shot. The fitted reference reaches +5 at sample 11. The stale reference reaches −5 at sample 20.](figures/decision-trace.svg)

## Does it hold across shots?

Both policies saw the same 4,000 synthetic shots with 35° of signal drift. Fitting the reference took 16 separate labeled measurements.

| Calibration | Incorrect reads | Correct resets within 120 ns |
| --- | ---: | ---: |
| Stale reference | 339 | 2,566 |
| Fitted reference | 16 | 3,427 |

**What if the signal drifts again?** In a separate Python replay, the phase shifted another 50° mid-run. A scheduled check detected it 64 shots later. Recalibration recovered 756 additional correct, on-time resets across 2,144 post-shift shots. [Explore the drift study](https://hanshengzhu0001.github.io/qubit-readout-lab/drift.html)

## Rust async: two runtimes, two jobs

**NeXosim** models one feedback loop in simulation time. A circuit actor prepares a Bell pair and measures its first qubit. A readout actor accumulates noisy voltage samples, then sends its decision through an async port. The circuit conditionally flips the partner and checks its final state and completion time.

**Tokio** runs independent benches concurrently. A semaphore limits execution to two benches, which share immutable inputs through an `Arc`. The runner checks 24,000 shot outcomes against a direct calculation. [Rust runner](examples/readout.rs)

## Where the idea came from

In my vision and inverse-PDE projects, I used a forward model to infer something hidden from partial observations. Here the unknown is the phase offset of a microwave signal. I fit it from labeled pilot shots, then test it on fresh measurements. The voltages and timing budget are synthetic. For the hardware context, see [Bengtsson et al. on readout optimization](https://arxiv.org/abs/2308.02079) and [Ali et al. on analog readout and recalibration](https://doi.org/10.1103/PhysRevApplied.22.044031).

<details>
<summary>Reproduce the experiments and figures</summary>

From the project directory, use Rust 1.93 and a Python environment:

```sh
python3 -m pip install -r requirements.txt
python3 scripts/readout.py
python3 scripts/animate.py
python3 scripts/impact.py
python3 scripts/workflow.py
python3 scripts/drift.py
python3 scripts/drift_animation.py
cargo test --locked
```

[Experiment data](figures/results.json) · [Inference and plotting](scripts/readout.py) · [Rust readout and Tokio runner](examples/readout.rs)

</details>
