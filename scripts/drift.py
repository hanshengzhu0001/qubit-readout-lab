"""Paired sequential-readout replay with an abrupt calibration change."""
import json
from pathlib import Path

import numpy as np

ROOT = Path(__file__).resolve().parents[1]
N, SHIFT, CADENCE, PILOTS = 4000, 1856, 128, 16
SIGMA, THRESHOLD, DEADLINE = .85, 5, 120
rng = np.random.default_rng(713)
t = np.arange(1, 49)
pole = .10 + np.array([-.14j, .14j])
templates = .10 * (1 - np.exp(-pole[:, None] * t)) / pole[:, None]
labels = rng.integers(0, 2, size=N)
pilot_labels = np.repeat([0, 1], PILOTS // 2)

def observe(states, phase):
    signal = templates[states] * np.exp(1j * np.deg2rad(phase))
    return signal + SIGMA * (rng.normal(size=signal.shape) + 1j * rng.normal(size=signal.shape))

def probe(phase):
    return float(np.rad2deg(np.angle(np.vdot(templates[pilot_labels], observe(pilot_labels, phase)))))

initial = probe(0)
fitted = initial
rows, checks = [], []
boundaries = sorted({*range(0, N, CADENCE), SHIFT, N})
for start, stop in zip(boundaries, boundaries[1:]):
    phase = 0 if start < SHIFT else 50
    if start and start % CADENCE == 0:
        estimate = probe(phase)
        gap = (estimate - fitted + 180) % 360 - 180
        refit = abs(gap) >= 18
        checks.append(dict(shot=start, estimate=round(estimate, 2), refit=refit))
        if refit:
            fitted = estimate
    states = labels[start:stop]
    volts = observe(states, phase)
    results = {}
    for name, angle in [("frozen", initial), ("adaptive", fitted)]:
        corrected = volts * np.exp(-1j * np.deg2rad(angle))
        odds = np.cumsum((abs(corrected-templates[0])**2 -
                          abs(corrected-templates[1])**2)/(2*SIGMA**2), axis=1)
        crossed = abs(odds) >= THRESHOLD
        index = np.where(crossed.any(axis=1), crossed.argmax(axis=1), len(t)-1)
        assigned = odds[np.arange(len(index)), index] > 0
        # Same feedback parameters as the Rust circuit. Correct assignment
        # produces target |0>; latency includes readout, message and gate slot.
        latency = 4*(index+1) + 16 + 12
        results[name] = int(np.sum((assigned == states) & (latency <= DEADLINE)))
    rows.append(dict(start=start, end=stop, phase=phase, fitted=round(fitted, 2), **results))

assert all(r["frozen"] == r["adaptive"] for r in rows if r["end"] <= SHIFT)
assert sum(r["end"]-r["start"] for r in rows) == N
summary = {key: sum(r[key] for r in rows if r["start"] >= SHIFT) for key in ("frozen","adaptive")}
detected = next(c["shot"] for c in checks if c["shot"] >= SHIFT and c["refit"])
data = dict(rows=rows, checks=checks, shift=SHIFT, detected=detected, summary=summary,
            post_shots=N-SHIFT, pilot_shots=PILOTS*(len(checks)+1), deadline=DEADLINE,
            initial=round(initial,2))
(ROOT/"figures/drift-results.json").write_text(json.dumps(data, indent=2)+"\n")
print(summary, "evaluations after drift", N-SHIFT, "first refit", detected, "pilot shots", data["pilot_shots"])
