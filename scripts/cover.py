"""Make the animated GitHub cover from the experiment's resonator equation."""
import cmath
from pathlib import Path

from PIL import Image, ImageDraw, ImageFilter, ImageFont

root = Path(__file__).resolve().parents[1]
def font(size):
    try:
        return ImageFont.truetype("DejaVuSans.ttf", size)
    except OSError:
        return ImageFont.load_default(size=size)

frames = []
colors = ("#8cf5dc", "#ffad86")
for frame in range(60):
    image = Image.new("RGB", (1000, 380), "#090d1a")
    draw = ImageDraw.Draw(image)
    for x in range(540, 1000, 28):
        draw.line((x, 18, x, 362), fill="#161e35")
    for y in range(18, 362, 28):
        draw.line((540, y, 990, y), fill="#161e35")
    draw.line((540, 190, 990, 190), fill="#364769")
    draw.line((635, 18, 635, 362), fill="#364769")
    glow = Image.new("RGB", image.size)
    halos = ImageDraw.Draw(glow)
    paths = []
    for state, color in enumerate(colors):
        pole = .1 + (2*state-1)*.14j
        path = [.1*(1-cmath.exp(-pole*t))/pole for t in range(49)]
        points = [(635+z.real*310,190-z.imag*205) for z in path]
        visible = points[:max(2,min(49,frame+1-state*6))]
        draw.line(points, fill="#34405a", width=2)
        halos.line(visible, fill=color, width=12)
        paths.append((visible,color,state))
    image = Image.blend(image, Image.composite(glow.filter(ImageFilter.GaussianBlur(11)), image,
                        glow.filter(ImageFilter.GaussianBlur(11)).convert("L")), .65)
    draw = ImageDraw.Draw(image)
    for visible,color,state in paths:
        draw.line(visible, fill=color, width=3)
        x,y = visible[-1]
        draw.ellipse((x-5,y-5,x+5,y+5),fill=color)
        draw.text((810, 70+state*230), f"STATE {state}",font=font(17),fill=color)
    draw.text((45, 48), "QUBIT READOUT LAB",font=font(18),fill="#a9dfff")
    draw.text((43, 100), "Read the",font=font(65),fill="#f2f5ff")
    draw.text((43, 176), "invisible.",font=font(65),fill="#b7a9ff")
    draw.text((47, 294), "How soon can you trust a qubit signal?",font=font(20),fill="#bac4df")
    frames.append(image)
frames[0].save(root/"figures/signal-state.gif",save_all=True,append_images=frames[1:],
               duration=[65]*59+[800],loop=0,optimize=True)
