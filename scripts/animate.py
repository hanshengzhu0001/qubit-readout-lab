"""Render a paired replay for the GitHub README from exported Rust traces."""
import json
from pathlib import Path

import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt
from matplotlib.animation import FuncAnimation, PillowWriter

root = Path(__file__).resolve().parents[1]
html = (root / "figures/readout-player.html").read_text()
data, _ = json.JSONDecoder().raw_decode(html.split("const data = ", 1)[1])
runs = {r["mode"]: r for r in data["runs"] if r["threshold"] == 5}
stale, fixed = runs["stale"], runs["corrected"]
idx = next((i for i, (a, b) in enumerate(zip(stale["previews"], fixed["previews"]))
            if a["decision"] != a["truth"] and b["decision"] == b["truth"]), 0)
pair = [r["previews"][idx] for r in (stale, fixed)]
end = max(len(shot["evidence"]) for shot in pair)

plt.style.use("dark_background")
fig, ax = plt.subplots(figsize=(8, 3.4), layout="constrained")
fig.patch.set_facecolor("#101826")
ax.set_facecolor("#101826")
ax.set(xlim=(0, end + 1), ylim=(-8, 8), xlabel="Samples observed", ylabel="Cumulative log odds",
       title="One voltage trace, two calibration choices")
for y in [-5, 5]:
    ax.axhline(y, color="#9baec9", ls="--", lw=1, alpha=.7)
ax.axhline(0, color="#435574", lw=1)
ax.spines[["top", "right"]].set_visible(False)
lines = [ax.plot([], [], color=color, lw=2.5, marker="o", markevery=[-1],
                 label=name)[0] for color, name in
         [("#ffad65", "Stale calibration"), ("#77e8c4", "Pilot corrected")]]
ax.legend(loc="upper left", frameon=False)
caption = ax.text(.99, .04, "", transform=ax.transAxes, ha="right", va="bottom", fontsize=9)

def update(frame):
    decisions = []
    for line, shot in zip(lines, pair):
        trace = shot["evidence"][:frame]
        line.set_data(range(len(trace)+1), [0]+trace)
        decisions.append(str(int(shot["decision"])) if frame >= len(shot["evidence"]) else "…")
    caption.set_text(f'True state {int(pair[0]["truth"])}  |  stale {decisions[0]}  |  corrected {decisions[1]}')
    return [*lines, caption]

frames = [0]*3 + list(range(1, end+1)) + [end]*12
FuncAnimation(fig, update, frames=frames, interval=125).save(
    root/"figures/readout-replay.gif", writer=PillowWriter(fps=8), dpi=105)
