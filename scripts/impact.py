"""Render the README experiment story from the exported Rust replay."""
import json
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont

ROOT = Path(__file__).resolve().parents[1]
player = (ROOT / "figures/readout-player.html").read_text()
data, _ = json.JSONDecoder().raw_decode(player.split("const data = ", 1)[1])
runs = {(r["mode"], r["threshold"]): r for r in data["runs"]}
early = [runs[mode, 5] for mode in ("stale", "corrected")]
full = runs["corrected", None]
pair = next((a, b) for a, b in zip(*(r["previews"] for r in early))
            if a["target_one_after"] and not b["target_one_after"])
shots = [*pair, full["previews"][early[1]["previews"].index(pair[1])]]
cohorts = [*early, full]
labels = ["Stale calibration", "Phase corrected", "Full window + correction"]
colors = ["#ffb287", "#79efd0", "#adb6ff"]
muted = "#aebbd2"
white = "#f1f6ff"
deadline = 120

def font(size, bold=False):
    names = ["DejaVuSans-Bold.ttf" if bold else "DejaVuSans.ttf",
             "/System/Library/Fonts/Supplemental/Arial Bold.ttf" if bold else
             "/System/Library/Fonts/Supplemental/Arial.ttf"]
    for name in names:
        try:
            return ImageFont.truetype(name, size)
        except OSError:
            pass
    return ImageFont.load_default(size=size)

def render(chapter, fraction):
    img = Image.new("RGB", (1000, 420), "#0a1020")
    d = ImageDraw.Draw(img)

    def text(x, y, value, size, color=white, bold=False):
        d.text((x, y), value, font=font(size, bold), fill=color)

    d.rounded_rectangle((17, 17, 983, 403), 24, fill="#101a30", outline="#273756", width=2)
    if chapter == 0:
        text(44, 38, "ONE SHOT  /  THREE WAYS TO READ IT", 16, "#94baf9", True)
        text(44, 68, "Can we reset the qubit in time?", 43, bold=True)
        now = 220 * fraction
        finish_x = lambda ns: 330 + ns / 220 * 415
        d.line((finish_x(deadline), 155, finish_x(deadline), 333), fill="#f2d482", width=2)
        text(finish_x(deadline)-57, 130, "120 ns deadline", 16, "#f2d482")
        for i, (label, color, shot) in enumerate(zip(labels, colors, shots)):
            y = 183+i*61
            final_ns = shot["finished_ns"]
            text(44, y-12, label, 23, color, True)
            d.rounded_rectangle((330,y,745,y+7), 4, fill="#263550")
            marker = finish_x(min(now, final_ns))
            d.line((330,y+3,marker,y+3), fill=color, width=5)
            d.ellipse((marker-6,y-2,marker+6,y+9), fill=color)
            if now >= final_ns:
                verdict = ("Wrong state" if shot["target_one_after"] else
                           "Correct and on time" if shot["on_time"] else "Correct but late")
                text(762, y-16, verdict, 19, color, True)
                text(762, y+9, f"{final_ns} ns", 15, muted)
        text(44, 360, "Bell pair → noisy measurement → classical decision → conditional gate → verify reset", 19, muted)
    else:
        text(44, 38, "4,000 MATCHED SYNTHETIC SHOTS", 16, "#94baf9", True)
        text(44, 68, "Count the resets that arrive on time.", 41, bold=True)
        for i, (label, color, run) in enumerate(zip(labels, colors, cohorts)):
            y = 166+i*60
            count = run["timely_resets"]
            text(44, y-12, label, 22, color, True)
            d.rounded_rectangle((330,y,745,y+12), 6, fill="#263550")
            if count:
                d.rounded_rectangle((330,y,330+415*count/run["shots"]*fraction,y+12), 6, fill=color)
            text(762,y-10,f"{round(count*fraction):,} / {run['shots']:,}",22,color,True)
        text(44, 360, "A phase fit from 16 labeled shots recovered 861 correct, on-time resets.", 21, muted)
    return img

frames = [render(0, i/25) for i in range(26)] + [render(1, i/16) for i in range(17)]
durations = [95]*26 + [75]*17
durations[0], durations[25], durations[-1] = 900, 3100, 4000
frames[0].save(ROOT/"figures/readout-impact.gif", save_all=True, append_images=frames[1:],
               duration=durations, loop=0, optimize=True)
