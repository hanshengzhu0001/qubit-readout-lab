"""Render the readout workflow for the README and the exhibit."""
from pathlib import Path
from PIL import Image, ImageDraw, ImageFont

ROOT = Path(__file__).resolve().parents[1]
WHITE, MUTED = "#f3f7ff", "#acbcd4"
MINT, AMBER, BLUE = "#7bf2d1", "#ffbe87", "#9bbdff"

def font(size, bold=False):
    for name in ("DejaVuSans-Bold.ttf" if bold else "DejaVuSans.ttf",
                 "/System/Library/Fonts/Supplemental/Arial Bold.ttf" if bold else
                 "/System/Library/Fonts/Supplemental/Arial.ttf"):
        try:
            return ImageFont.truetype(name, size)
        except OSError:
            continue
    return ImageFont.load_default(size=size)

def frame(step):
    image = Image.new("RGB", (1000, 430), "#000000")
    d = ImageDraw.Draw(image)

    def text(x, y, value, size, color=WHITE, bold=False):
        d.text((x, y), value, fill=color, font=font(size, bold))

    def arrow(x, y, end, color):
        d.line((x, y, end-4, y), fill=color, width=3)
        d.polygon(((end-8, y-5), (end, y), (end-8, y+5)), fill=color)

    def card(x, y, w, label, detail, active, color):
        edge = color if active else "#262e3a"
        d.rounded_rectangle((x, y, x+w, y+65), 10, outline=edge,
                            fill="#0d131e" if active else "#080b10", width=2)
        text(x+12, y+9, label, 19, color if active else MUTED, True)
        text(x+12, y+36, detail, 15, WHITE if active else MUTED)

    text(40, 26, "SIGNAL / STATE", 16, BLUE, True)
    text(40, 53, "One measurement. Two possible resets.", 42, bold=True)
    text(41, 112, "Same synthetic shot, same target, different calibration.", 20, MUTED)
    card(40, 156, 278, "Prepare and measure", "The measured qubit returns 1", step >= 0, BLUE)
    card(361, 156, 278, "Read the microwave signal", "Phase drift shifts the fingerprint", step >= 1, BLUE)
    card(682, 156, 278, "Choose a feedback action", "A reported 1 triggers a target flip", step >= 2, BLUE)
    arrow(325, 189, 352, BLUE)
    arrow(646, 189, 673, BLUE)
    d.line([(820,221),(820,232),(22,232),(22,369)], fill="#49566c", width=2)
    arrow(22, 282, 38, AMBER if step >= 3 else "#49566c")
    arrow(22, 369, 38, MINT if step >= 4 else "#49566c")
    for y, color, label, call, action, outcome, start in [
        (250, AMBER, "Stale calibration", "Calls 0", "Skips the flip", "Target stays |1⟩", 3),
        (337, MINT, "After recalibration", "Calls 1", "Applies the flip", "Target resets to |0⟩", 4),
    ]:
        card(40,y,300,label,call,step>=start,color)
        card(390,y,248,"Controller feedback",action,step>=start,color)
        card(688,y,272,"Final target state",outcome,step>=start,color)
        arrow(350,y+32,380,color if step>=start else "#49566c")
        arrow(648,y+32,678,color if step>=start else "#49566c")
    return image

frames = [frame(i) for i in range(5)]
frames[-1].save(ROOT/"figures/workflow.png")
frames[0].save(ROOT/"figures/workflow.gif", save_all=True, append_images=frames[1:],
               duration=[1350,1400,1500,2200,6500], loop=0, optimize=True)
