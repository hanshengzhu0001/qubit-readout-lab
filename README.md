# Signal / State · Qubit Readout Lab

**Can a quantum controller read a qubit and reset its partner before the deadline?**

A noisy measurement can fool an early controller. Waiting for the full signal improves accuracy, but leaves less time to act. This lab follows that tradeoff through an entire two-qubit feedback circuit.

<a href="https://hanshengzhu0001.github.io/qubit-readout-lab/machine.html"><img src="figures/readout-impact.gif" width="100%" alt="Animated comparison of three policies on the same shot, followed by on-time reset counts across 4,000 synthetic shots"></a>

[**Tour the quantum computer →**](https://hanshengzhu0001.github.io/qubit-readout-lab/machine.html) · [Inspect the voltage traces](https://hanshengzhu0001.github.io/qubit-readout-lab/exhibit.html#experiment) · [Rust async](#rust-async-two-runtimes-two-jobs)

**Why it matters.** The controller’s job is to finish the correct operation in time. With an illustrative **120 ns** budget and 35° of synthetic phase drift, a calibration fit from **16 labeled shots** recovered **861 correct, on-time resets** in a separate set of 4,000 shots. Holding out for all 48 samples produced just one assignment error after correction, but every shot finished at **220 ns**.

The animation first shows a selected shot, then the aggregate over all matched shots. The voltages and timings are simulated.

## From a quantum circuit to a classical decision and back

The simulator prepares a Bell pair from `|00⟩` with H and CNOT. Measuring the first qubit takes one of the two equally probable branches. The experiment explicitly supplies balanced branches so every calibration policy sees the same synthetic shots. A readout actor receives only the voltage-derived evidence. It sends its decision back to the circuit actor, which applies X to the second qubit if the controller reports 1, then checks its final state.

```mermaid
flowchart LR
  A[Prepare Bell pair] --> B[Measure first qubit]
  B --> C[Readout actor accumulates I/Q evidence]
  C -->|async decision message| D[Circuit actor applies conditional X]
  D --> E[Verify target state and completion time]
```

With an illustrative **120 ns deadline**, early readout reset the target correctly and on time in **2,566 / 4,000** drifted shots. Pilot correction raised that to **3,427 / 4,000**. The 48-sample policy finishes at 220 ns under the same timing model. The deadline scores the outcome and the circuit still finishes late shots.

This is a Bell-pair feedback experiment with an explicit two-qubit state vector and synthetic analog readout. H, CNOT, projective measurement, and conditional X are modeled directly. Shots run serially inside each bench. Tokio runs separate benches concurrently.

## A small inverse problem before each readout experiment

The idea comes from two prior projects. Single-image scene reconstruction uses an observation to infer hidden structure. Inverse-PDE work uses known dynamics to constrain what can explain partial observations. Here the hidden quantity is a shared phase offset, and the forward model is a driven resonator. With eight labeled calibration shots per state, we fit one rotation by minimizing squared error between the predicted and measured I/Q trajectories. We then freeze it and evaluate on 4,000 separate shots.

| Condition at threshold 5 | Mean samples | Errors out of 4,000 |
|---|---:|---:|
| Nominal | 18.54 | 10 |
| 35° drift, stale calibration | 21.76 | 339 |
| 35° drift, fitted calibration | 18.64 | 16 |

The fitted angle was 42.99°. Even with that residual error, correction recovered most of the lost assignment accuracy in this seeded run. The experiment assumes one constant phase offset shared by pilot and evaluation shots. It does not establish robustness to drift within a run or other hardware effects.

The forward model solves a driven, damped linear resonator with state-dependent detuning. A Gaussian likelihood compares each new pair of voltages with the two predicted positions and accumulates evidence. Each comparison uses identical shots and noise across stopping rules. The two classes have equal prior probability. The fixed-window baseline uses the same likelihood at sample 48.

## Replay the experiment

[Open the interactive player](figures/readout-player.html). On GitHub, download the HTML and open it locally. Choose a calibration mode, stopping rule, and shot. Play or scrub to see the measured I/Q path, cumulative log odds, state decision, and simulated feedback time.

## Accuracy versus readout length

<img src="figures/readout.png" width="100%" alt="Resonator paths and assignment accuracy versus mean samples under nominal, stale, and corrected calibration">

The chart changes the stopping threshold on the same shots. [Watch the evidence cross the decision threshold](figures/readout-replay.gif). For each curve, the rightmost point uses all 48 samples.

## Rust async: two runtimes, two jobs

**Tokio coordinates experiments. NeXosim schedules the events inside each experiment.**

- **Ownership and borrowing.** An immutable `Arc<Input>` shares the fixture among runs. Each bench borrows its shots and owns its actor state. The sample event moves its payload into the next scheduled event.
- **Tokio.** `JoinSet::spawn_blocking` runs independent synchronous benches while the async coordinator waits on an owned semaphore permit. At most two benches run concurrently.
- **NeXosim.** Its executor advances logical time one sample event at a time. The actor updates its log odds and schedules feedback once it hits the threshold or uses all 48 samples. Its output port sends the decision with `.await`. The illustrative timing uses a 4 ns sample period and 16 ns feedback delay.
- **Verification.** Rust fits the phase and checks each of 24,000 simulated decisions, final target states, feedback timestamps and deadlines against a direct calculation. Python generates the fixture and compares aggregate accuracy and sample counts with its vectorized reference.

The data are synthetic and assume a fixed state within each shot, independent Gaussian noise, and a phase offset shared across pilot and test shots. The timing model processes one shot at a time.

## Reproduce locally

From the project directory, use Rust 1.93 and a Python environment:

```sh
python3 -m pip install -r requirements.txt
python3 scripts/readout.py
python3 scripts/animate.py
python3 scripts/impact.py
cargo test --locked
```

[Experiment data](figures/results.json) · [Inference and plotting](scripts/readout.py) · [Rust readout and Tokio runner](examples/readout.rs)

## Research context

Model-based readout optimization has been demonstrated on superconducting hardware. Sequential detection has also been evaluated offline on measured shots. This small experiment borrows those ideas to study calibration mismatch and feedback timing. [Bengtsson et al.](https://arxiv.org/abs/2308.02079) · [Johnson et al.](https://arxiv.org/abs/2610.00783)
