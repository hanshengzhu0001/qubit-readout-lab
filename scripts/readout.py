"""Seeded paired readout experiment and NeXosim feedback replay."""
from pathlib import Path
import subprocess
import tempfile
import json
import numpy as np
import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt

root = Path(__file__).resolve().parents[1]
rng = np.random.default_rng(2120)
t = np.arange(1, 49)
# Complex ring-up under constant drive, with opposite state-dependent detunings.
def response(sign):
    pole = 0.10 + sign * 0.14j
    return 0.10 * (1 - np.exp(-pole*t)) / pole

a, b = response(-1), response(1)
labels = np.repeat([False, True], 2000)
clean = np.where(labels[:, None], b, a)
sigma = 0.85
noise = sigma * (rng.normal(size=clean.shape) + 1j*rng.normal(size=clean.shape))
fig, axes = plt.subplots(1, 2, figsize=(10, 4.5), layout="constrained")
for curve, label in [(a, 'State 0'), (b, 'State 1')]:
    axes[0].plot(curve.real, curve.imag, label=label, lw=3)
    axes[0].scatter(curve.real[::6], curve.imag[::6], s=20)
axes[0].set(xlabel="In-phase voltage (scaled)", ylabel="Quadrature voltage (scaled)", title="Two candidate resonator paths")
axes[0].legend()
rows = []
# Independent, labeled pilot shots precede the held-out evaluation.
pilot_rng = np.random.default_rng(2121)
pilot_templates = np.repeat(np.stack([a, b]), 8, axis=0)
for rotation, corrected, marker, legend in [
    (0, False, 'o', 'Nominal'),
    (35, False, 's', '35° drift'),
    (35, True, '^', '35° drift + pilot correction'),
]:
    z = clean*np.exp(1j*np.deg2rad(rotation)) + noise
    estimated_phase = 0.0
    if corrected:
        pilot_noise = sigma * (pilot_rng.normal(size=pilot_templates.shape)
                               + 1j * pilot_rng.normal(size=pilot_templates.shape))
        pilots = pilot_templates * np.exp(1j*np.deg2rad(rotation)) + pilot_noise
        # Least squares over unit-magnitude rotations.
        estimated_phase = np.angle(np.vdot(pilot_templates, pilots))
        z *= np.exp(-1j * estimated_phase)
    if corrected:
        pilot_measurements = pilots
    # Equal priors, independent Gaussian I/Q noise. Compare squared residuals.
    odds = np.cumsum((abs(z-a)**2 - abs(z-b)**2)/(2*sigma**2), axis=1)
    group = []
    for threshold in [2, 3, 4, 5, 7, 10, np.inf]:
        hit = abs(odds) >= threshold
        stop = np.where(hit.any(axis=1), hit.argmax(axis=1), len(t)-1)
        correct = (odds[np.arange(len(stop)), stop] > 0) == labels
        row = dict(rotation_deg=rotation, corrected=corrected,
                   estimated_phase_deg=float(np.rad2deg(estimated_phase)), threshold=float(threshold) if np.isfinite(threshold) else None,
                   mean_samples=float((stop+1).mean()), accuracy=float(correct.mean()))
        rows.append(row)
        group.append(row)
    axes[1].plot([r['mean_samples'] for r in group], [100*r['accuracy'] for r in group],
                 marker=marker, label=legend)
axes[1].set(xlabel="Mean samples used per shot", ylabel="Correct assignments (%)", title="Earlier decisions under calibration drift")
axes[1].legend()
for ax in axes:
    ax.spines[['top','right']].set_visible(False)
    ax.grid(alpha=.16)
fig.savefig(root/'figures/readout.png', dpi=160)
# The Rust example independently fits the phase, assigns states, and checks feedback.
def iq(values):
    return np.stack([values.real, values.imag], axis=-1).tolist()

def shots(values, states):
    return [dict(samples=x, state=bool(state)) for x, state in zip(iq(values), states)]

with tempfile.TemporaryDirectory() as tmp:
    inputs = Path(tmp)/"shots.json"
    inputs.write_text(json.dumps(dict(
        templates=iq(np.stack([a,b])), sigma=sigma,
        nominal=shots(clean+noise, labels),
        drift=shots(clean*np.exp(1j*np.deg2rad(35))+noise, labels),
        pilots=shots(pilot_measurements, np.repeat([False,True], 8)),
    )))
    output = subprocess.check_output(
        ["cargo", "run", "--quiet", "--locked", "--example", "readout", "--", str(inputs)],
        cwd=root, text=True,
    )
data = json.loads(output)
for run in data["runs"]:
    rotation = 0 if run["mode"] == "nominal" else 35
    expected = next(row for row in rows if row["rotation_deg"] == rotation
                    and row["corrected"] == (run["mode"] == "corrected")
                    and row["threshold"] == run["threshold"])
    assert run["errors"] == round((1 - expected["accuracy"]) * len(labels)), ({k:v for k,v in run.items() if k != "previews"}, expected)
    assert abs(run["mean_samples"] - expected["mean_samples"]) < 1e-10
    print({k: v for k, v in run.items() if k != "previews"})
(root/"figures/results.json").write_text(json.dumps(rows, indent=2)+"\n")
template = (root/"scripts/player.html").read_text()
(root/"figures/readout-player.html").write_text(
    template.replace("__READOUT_DATA__", json.dumps(data, separators=(",", ":")))
)
