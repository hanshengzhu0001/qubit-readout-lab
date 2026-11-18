# Qubit Readout Lab

**How early can a controller trust a qubit readout?**

A superconducting readout produces two voltage channels, I and Q. As the resonator rings up, two candidate qubit states trace different paths in that plane. This experiment asks how much of a noisy path a classical controller needs to see before acting.

![Animated comparison of accumulated evidence on the same synthetic shot with stale and corrected phase calibration](figures/readout-replay.gif)

[Interactive replay](figures/readout-player.html) · [Experiment data](figures/results.json) · [Rust implementation](examples/readout.rs)

With a 35° phase shift, the early decision rule made **339 errors in 4,000 synthetic shots**. Estimating one phase offset from **16 labeled calibration shots** brought that down to **16 errors**, with a mean decision time of **18.64 samples out of 48**.

## A small inverse problem before each readout experiment

The idea comes from two prior projects. Single-image scene reconstruction uses an observation to infer hidden structure. Inverse-PDE work uses known dynamics to constrain what can explain partial observations. Here the hidden quantity is a shared phase offset, and the forward model is a driven resonator. With eight labeled calibration shots per state, we fit one rotation by minimizing squared error between the predicted and measured I/Q trajectories. We then freeze it and evaluate on 4,000 separate shots.

| Condition at threshold 5 | Mean samples | Errors out of 4,000 |
|---|---:|---:|
| Nominal | 18.54 | 10 |
| 35° drift, stale calibration | 21.76 | 339 |
| 35° drift, fitted calibration | 18.64 | 16 |

The fitted angle was 42.99°. Even with that residual error, correction recovered most of the lost assignment accuracy in this seeded run. The experiment assumes one constant phase offset shared by pilot and evaluation shots. It does not establish robustness to drift within a run or other hardware effects.

The forward model solves a driven, damped linear resonator with state-dependent detuning. A Gaussian likelihood compares each new pair of voltages with the two predicted positions and accumulates evidence. Each comparison uses identical shots and noise across stopping rules. The two classes have equal prior probability. The fixed-window baseline uses the same likelihood at sample 48.

[Open the animated readout replay](figures/readout-player.html). On GitHub, download the HTML and open it locally. Choose a calibration mode, stopping rule, and shot. Play or scrub to see the measured I/Q path, cumulative log odds, state decision, and simulated feedback time.

## Accuracy versus readout length

![State-dependent resonator paths and assignment accuracy versus mean samples for nominal, stale, and corrected calibration](figures/readout.png)

The chart changes the stopping threshold on the same shots. For each curve, the rightmost point uses all 48 samples.

## What Rust does

- **Ownership and borrowing.** An immutable `Arc<Input>` shares the fixture among runs. Each bench borrows its shots and owns its actor state. The sample event moves its payload into the next scheduled event.
- **Tokio.** `JoinSet::spawn_blocking` runs independent synchronous benches while the async coordinator waits on an owned semaphore permit. At most two benches run concurrently.
- **NeXosim.** Its executor advances logical time one sample event at a time. The actor updates its log odds and schedules feedback once it hits the threshold or uses all 48 samples. Its output port sends the decision with `.await`. The illustrative timing uses a 4 ns sample period and 16 ns feedback delay.
- **Verification.** Rust fits the phase and checks each of 24,000 simulated decisions and feedback timestamps against a direct calculation. Python generates the fixture and compares aggregate accuracy and sample counts with its vectorized reference.

The data are synthetic and assume a fixed state within each shot, independent Gaussian noise, and a phase offset shared across pilot and test shots. The timing model processes one shot at a time.

## Reproduce locally

From the project directory, use Rust 1.93 and a Python environment:

```sh
python3 -m pip install -r requirements.txt
python3 scripts/readout.py
python3 scripts/animate.py
cargo test --locked
```

[Experiment data](figures/results.json) · [Inference and plotting](scripts/readout.py) · [Rust readout and Tokio runner](examples/readout.rs)

## Research context

Model-based readout optimization has been demonstrated on superconducting hardware. Sequential detection has also been evaluated offline on measured shots. This small experiment borrows those ideas to study calibration mismatch and feedback timing. [Bengtsson et al.](https://arxiv.org/abs/2308.02079) · [Johnson et al.](https://arxiv.org/abs/2610.00783)
