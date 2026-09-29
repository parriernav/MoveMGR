from pathlib import Path
from PIL import Image, ImageDraw

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "src-tauri" / "icons"
OUT.mkdir(parents=True, exist_ok=True)

def icon(size: int) -> Image.Image:
    scale = size / 256
    image = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    draw = ImageDraw.Draw(image)
    def box(coords, radius, fill):
        draw.rounded_rectangle(tuple(round(value * scale) for value in coords), radius=round(radius * scale), fill=fill)
    box((8, 8, 248, 248), 56, "#182B52")
    draw.polygon([(round(x*scale), round(y*scale)) for x,y in [(42,67),(87,67),(101,83),(140,83),(140,153),(128,165),(54,165),(42,153)]], fill="#7194F4")
    box((42, 71, 140, 165), 12, "#7194F4")
    draw.polygon([(round(x*scale), round(y*scale)) for x,y in [(116,119),(157,119),(171,135),(214,135),(214,193),(202,205),(128,205),(116,193)]], fill="#BDECD9")
    box((116, 123, 214, 205), 12, "#BDECD9")
    width = max(2, round(15 * scale))
    points = [(76,118),(153,118),(133,98),(153,118),(133,138)]
    draw.line([(round(x*scale),round(y*scale)) for x,y in points[:2]], fill="white", width=width)
    draw.line([(round(x*scale),round(y*scale)) for x,y in points[2:]], fill="white", width=width, joint="curve")
    return image

for name, size in [("32x32.png",32),("128x128.png",128),("128x128@2x.png",256)]:
    icon(size).save(OUT / name)
master = icon(1024)
master.save(OUT / "icon.png")
master.save(OUT / "icon.ico", sizes=[(16,16),(24,24),(32,32),(48,48),(64,64),(128,128),(256,256)])
master.save(OUT / "icon.icns", sizes=[(16,16),(32,32),(64,64),(128,128),(256,256),(512,512),(1024,1024)])
print(f"Generated icons in {OUT}")

