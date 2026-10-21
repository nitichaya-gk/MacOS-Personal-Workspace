"""Generates the 1024px source app icon (scripts/app-icon.png). Run `npx tauri icon scripts/app-icon.png` afterwards."""
from PIL import Image, ImageDraw

S = 1024
img = Image.new("RGBA", (S, S), (0, 0, 0, 0))
# macOS icon grid: ~824px rounded square centred on the canvas.
bg = Image.new("RGBA", (S, S))
d = ImageDraw.Draw(bg)
for y in range(S):
    t = y / S
    d.line([(0, y), (S, y)], fill=(int(48 + 30 * t), int(54 + 20 * t), int(72 + 40 * t), 255))
mask = Image.new("L", (S, S), 0)
ImageDraw.Draw(mask).rounded_rectangle([100, 100, 924, 924], radius=185, fill=255)
img.paste(bg, (0, 0), mask)
d = ImageDraw.Draw(img)
# Page with blocks + a connected node: notes, database and canvas in one.
d.rounded_rectangle([250, 230, 640, 790], radius=40, fill=(250, 250, 248, 255))
for i, w in enumerate([280, 220, 250, 160]):
    y = 320 + i * 95
    d.rounded_rectangle([300, y, 300 + w, y + 42], radius=14, fill=(206, 210, 220, 255) if i else (120, 132, 255, 255))
d.line([(640, 470), (720, 470), (720, 360)], fill=(250, 250, 248, 255), width=22)
d.rounded_rectangle([660, 250, 790, 370], radius=30, fill=(255, 196, 92, 255))
d.ellipse([665, 560, 795, 690], fill=(96, 214, 168, 255))
d.line([(640, 625), (665, 625)], fill=(250, 250, 248, 255), width=22)
img.save("scripts/app-icon.png")
