"""Render a black-background storyboard from the drift experiment results."""
import json
from pathlib import Path
from PIL import ImageFont
from storyboard import Storyboard

root = Path(__file__).resolve().parents[1]
data = json.loads((root / "figures/drift-results.json").read_text())
rows = data["rows"]
shift, detected = data["shift"], data["detected"]
mint, amber, pale = "#7bf2d1", "#ffbf79", "#c4d0e3"

def font(size, bold=False):
    size *= Storyboard.scale
    for name in ("DejaVuSans-Bold.ttf" if bold else "DejaVuSans.ttf",
                 "/System/Library/Fonts/Supplemental/Arial Bold.ttf" if bold else
                 "/System/Library/Fonts/Supplemental/Arial.ttf"):
        try:
            return ImageFont.truetype(name, size)
        except OSError:
            continue
    return ImageFont.load_default(size=size)

scenes = [
    ("The first calibration works", "16 labeled shots establish the initial phase.", 1536),
    ("Then the signal rotates again", "The synthetic signal shifts by 50° after shot 1,856.", shift),
    ("The controller has stale calibration", "64 evaluation shots pass before the next labeled check.", detected-1),
    ("A labeled check detects the shift", "At 1,920 completed shots, a fresh pilot batch updates the phase.", 2176),
    ("Correct, on-time resets recover", "Both policies see the same evaluation shots and share a 120 ns deadline.", 4000),
]

def frame(chapter, limit):
    d = Storyboard(1000, 448)
    im = d.image
    def text(x,y,value,size,color=pale,bold=False):
        d.text((x,y),value,font=font(size,bold),fill=color)
    title, caption, _ = scenes[chapter]
    text(42,28,"WHEN CALIBRATION DRIFTS AGAIN",16,mint,True)
    text(42,54,title,39,"#ffffff",True)
    text(43,108,caption,20)
    if chapter < 4:
        text(42,148,"CORRECT AND ON TIME PER BATCH",14,pale,True)
        for pct in (0,50,100):
            y=326-pct*1.44
            d.line((95,y,946,y),fill="#202935",width=1)
            text(43,y-8,f"{pct}%",13)
        for event,color in ((shift,amber),(detected,mint)):
            if event <= limit:
                x=95+851*event/4000
                d.line((x,180,x,326),fill=color,width=2)
        for key,color in (("frozen",amber),("adaptive",mint)):
            pts=[(95+851*(r["start"]+r["end"])/8000,
                  326-144*r[key]/(r["end"]-r["start"])) for r in rows if r["end"] <= limit]
            if len(pts)>1:
                d.line(pts,fill=color,width=4)
            if pts:
                x,y=pts[-1]
                d.ellipse((x-5,y-5,x+5,y+5),fill=color)
        text(94,337,"0",14)
        text(787,337,"4,000 evaluation shots",14)
        text(43,397,"● Frozen calibration",18,amber,True)
        text(292,397,"● Periodic checks + recalibration",18,mint,True)
    else:
        for i,(key,color,label) in enumerate((("frozen",amber,"Frozen calibration"),("adaptive",mint,"Checks + recalibration"))):
            y=179+i*79
            n=data["summary"][key]
            text(43,y,label,24,color,True)
            d.rounded_rectangle((345,y+7,760,y+19),6,fill="#242d39")
            d.rounded_rectangle((345,y+7,345+415*n/data["post_shots"],y+19),6,fill=color)
            text(780,y-1,f"{n:,} / {data['post_shots']:,}",23,color,True)
        text(43,358,f'+{data["summary"]["adaptive"]-data["summary"]["frozen"]} correct, on-time resets after drift',27,mint,True)
        text(44,402,"512 labeled shots total for ongoing checks, including the initial 16.",17)
    return im

frames, durations = [], []
previous=0
for i,(_,_,end) in enumerate(scenes):
    count=12 if i<4 else 1
    for j in range(count):
        frames.append(frame(i,round(previous+(end-previous)*(j+1)/count)))
        durations.append(2800 if j==count-1 else 85)
    previous=end
durations[-1]=5200
frames[0].save(root/"figures/drift-recovery.gif",save_all=True,append_images=frames[1:],
               duration=durations,loop=0,optimize=True)
