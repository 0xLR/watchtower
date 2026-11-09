"""Generate the scaffold's geometric app mark (requires Pillow)."""
from pathlib import Path
from PIL import Image, ImageDraw

out = Path(__file__).resolve().parents[1] / 'apps/desktop/src-tauri/icons'
out.mkdir(parents=True, exist_ok=True)
image = Image.new('RGBA', (256, 256), '#0b0d0f')
draw = ImageDraw.Draw(image)
draw.rectangle((48, 48, 208, 208), outline='#d9a441', width=8)
draw.line((80, 96, 96, 160, 128, 112, 160, 160, 176, 96), fill='#e5e7e9', width=10)
image.save(out / 'icon.png')
image.save(out / 'icon.ico', sizes=[(16, 16), (32, 32), (48, 48), (256, 256)])

image.save(out / 'icon.icns')
