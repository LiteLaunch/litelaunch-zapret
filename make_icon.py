"""Generates the plugin's icons: icon.png and zapret.ico (a white shield with a check on green, the
plugin and its running config) and one icon per row kind in icons/: a white sign on a colored
rounded square, the same look as the launcher's own icon."""
import math
from pathlib import Path
from PIL import Image, ImageDraw

S = 1024
W = (255, 255, 255, 255)
HERE = Path(__file__).parent
OUT = HERE / "icons"


def base(color):
    img = Image.new("RGBA", (S, S), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)
    d.rounded_rectangle((32, 32, S - 32, S - 32), radius=220, fill=color)
    return img, d


def icon(name, color, draw):
    img, d = base(color)
    draw(d, color)
    img.resize((256, 256), Image.LANCZOS).save(OUT / f"{name}.png")
    return img


def shield(d, c):
    d.polygon([(512, 190), (790, 290), (780, 520), (720, 680), (620, 790), (512, 850),
               (404, 790), (304, 680), (244, 520), (234, 290)], fill=W)
    d.line([(392, 520), (482, 615), (640, 420)], fill=c, width=80, joint="curve")


def star(d, c):
    pts = []
    for i in range(10):
        r = 300 if i % 2 == 0 else 128
        a = -math.pi / 2 + i * math.pi / 5
        pts.append((512 + r * math.cos(a), 540 + r * math.sin(a)))
    d.polygon(pts, fill=W)


def gear(d, c):
    for i in range(8):
        a = i * math.pi / 4
        dx, dy = math.cos(a), math.sin(a)
        px, py = -dy, dx
        cx, cy = 512 + dx * 300, 512 + dy * 300
        w = h = 70
        d.polygon([(cx + px * w - dx * h, cy + py * w - dy * h), (cx - px * w - dx * h, cy - py * w - dy * h),
                   (cx - px * w + dx * h, cy - py * w + dy * h), (cx + px * w + dx * h, cy + py * w + dy * h)], fill=W)
    d.ellipse((262, 262, 762, 762), fill=W)
    d.ellipse((412, 412, 612, 612), fill=c)


def folder(d, c):
    d.rounded_rectangle((250, 300, 520, 420), radius=40, fill=W)
    d.rounded_rectangle((250, 360, 774, 724), radius=50, fill=W)


def folder_pick(d, c):
    folder(d, c)
    d.rectangle((492, 450, 532, 650), fill=c)
    d.rectangle((412, 530, 612, 570), fill=c)


def download(d, c):
    d.rectangle((462, 230, 562, 520), fill=W)
    d.polygon([(330, 500), (694, 500), (512, 690)], fill=W)
    d.rounded_rectangle((270, 730, 754, 800), radius=30, fill=W)


def listing(d, c):
    for y in (330, 490, 650):
        d.ellipse((270, y - 45, 360, y + 45), fill=W)
        d.rounded_rectangle((410, y - 40, 760, y + 40), radius=40, fill=W)


def warn(d, c):
    d.rounded_rectangle((462, 250, 562, 600), radius=50, fill=W)
    d.ellipse((457, 660, 567, 770), fill=W)


OUT.mkdir(exist_ok=True)
# The plugin itself (keyword hint, notifications) and a running config.
plugin = icon("running", (22, 140, 90, 255), shield)
plugin.resize((256, 256), Image.LANCZOS).save(HERE / "icon.png")
plugin.save(HERE / "zapret.ico", sizes=[(16, 16), (24, 24), (32, 32), (48, 48), (64, 64), (256, 256)])

icon("stop", (205, 52, 52, 255), lambda d, c: d.rounded_rectangle((322, 322, 702, 702), radius=60, fill=W))
icon("config", (72, 98, 140, 255), lambda d, c: d.polygon([(400, 290), (400, 734), (760, 512)], fill=W))
icon("favorite", (222, 150, 20, 255), star)
icon("settings", (88, 92, 100, 255), gear)
icon("folder", (226, 118, 34, 255), folder)
icon("folder-pick", (24, 128, 140, 255), folder_pick)
icon("update", (32, 110, 210, 255), download)
icon("list", (120, 80, 190, 255), listing)
icon("warning", (150, 40, 40, 255), warn)
print("ok")
