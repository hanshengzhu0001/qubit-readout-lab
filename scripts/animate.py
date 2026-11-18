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
            if a["decision"] != a["truth"] and b["decision"] == b["truth"]))
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

# A short visual story for readers who have never seen an I/Q plot.
from matplotlib.patches import FancyBboxPatch

fig, hero = plt.subplots(figsize=(9, 4.6))
fig.subplots_adjust(0, 0, 1, 1)
fig.patch.set_facecolor("#0b1220")
hero.set(xlim=(0, 900), ylim=(0, 460))
hero.axis("off")
ink = "#f1f6ff"
muted = "#a9b9d0"

def story(frame):
    hero.clear()
    hero.set(xlim=(0, 900), ylim=(0, 460))
    hero.axis("off")
    hero.text(48, 408, "QUBIT READOUT LAB  /  SYNTHETIC EXPERIMENT", color="#8bb9ff", fontsize=10)
    hero.text(48, 350, "Same signal. Two decisions.", color=ink, fontsize=29, weight="bold")
    hero.text(48, 304, "Can the controller still act early when its calibration drifts?", color=muted, fontsize=13)
    for x, title, shot, accent in [
        (48, "STALE CALIBRATION", pair[0], "#ffad78"),
        (462, "PHASE CORRECTED", pair[1], "#74e5b9"),
    ]:
        hero.add_patch(FancyBboxPatch((x, 102), 390, 156, boxstyle="round,pad=0,rounding_size=13",
                                      facecolor="#172438", edgecolor="#30425e"))
        hero.text(x+21, 220, title, color=accent, fontsize=10, weight="bold")
        count = len(shot["evidence"])
        decided = frame >= count
        message = f'Calls state {int(shot["decision"])}' if decided else "Listening…"
        hero.text(x+21, 166, message, color=ink, fontsize=21, weight="bold")
        if decided:
            verdict = "CORRECT" if shot["decision"] == shot["truth"] else "WRONG"
            hero.text(x+365, 220, verdict, color=accent, fontsize=9, ha="right", weight="bold")
        hero.text(x+21, 128, f"{min(frame,count):02d} samples used", color=muted, fontsize=10)
        hero.plot([x+151, x+151+210*min(frame,count)/end], [133, 133],
                  color=accent, lw=4, solid_capstyle="round")
    footer = f'Prepared state: {int(pair[0]["truth"])}'
    if frame >= end:
        footer += "     •     16 labeled shots were used to fit the phase correction"
    hero.text(48, 55, footer, color=muted, fontsize=11)

FuncAnimation(fig, story, frames=[0]*10 + list(range(1, end+1)) + [end]*22,
              interval=140).save(root/"figures/readout-story.gif", writer=PillowWriter(fps=7), dpi=100)
