#!/usr/bin/env python3
"""Convert the explicitly synthetic native audition PPMs into local review media.
Requires Pillow. No network calls, capture devices, or application settings.
"""
import argparse
from pathlib import Path
from PIL import Image, ImageDraw

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('directory', type=Path)
args = parser.parse_args()
root = args.directory
names = ['Bass Web', 'Ribbon Reactor', 'Shockwave Tunnel', 'Aurora Strings', 'Prism Surge', 'Faultline']
sheet = Image.new('RGB', (1280, 1152), '#090d16')
draw = ImageDraw.Draw(sheet)
for index, name in enumerate(names):
    scene = index + 26
    paths = sorted(root.glob(f'scene-{scene}-*.ppm'))
    if len(paths) != 180:
        raise SystemExit(f'Expected 180 frames for {name}, found {len(paths)}')
    frames = [Image.open(path).convert('RGB') for path in paths]
    gif_frames = [frame.quantize(colors=128, method=Image.Quantize.FASTOCTREE) for frame in frames]
    gif_frames[0].save(root / f'scene-{scene}.gif', save_all=True, append_images=gif_frames[1:], duration=67, loop=0, optimize=False)
    still = frames[135].resize((640, 360))
    still.save(root / f'scene-{scene}.png')
    thumbnail = still.resize((608, 342))
    x, y = index % 2 * 640 + 16, index // 2 * 384
    sheet.paste(thumbnail, (x, y + 32))
    draw.text((x, y + 10), name, fill='white')
sheet.save(root / 'contact-sheet.png')
transitions = [Image.open(path).quantize(colors=128, method=Image.Quantize.FASTOCTREE) for path in sorted(root.glob('transition-*.ppm'))]
if transitions:
    transitions[0].save(root / 'transition.gif', save_all=True, append_images=transitions[1:], duration=67, loop=0)
mixed = [Image.open(path).quantize(colors=128, method=Image.Quantize.FASTOCTREE) for path in sorted(root.glob('mixed-transition-*.ppm'))]
if mixed:
    mixed[0].save(root / 'mixed-transition.gif', save_all=True, append_images=mixed[1:], duration=67, loop=0)
# A static filmstrip makes temporal artifacts and event recovery easy to inspect.
for scene in range(26, 32):
    storyboard = Image.new('RGB', (1280, 360), '#090d16')
    for index, frame in enumerate([25, 55, 85, 115, 125, 135, 150, 175]):
        image = Image.open(root / f'scene-{scene}-{frame:03}.ppm').resize((320, 180))
        storyboard.paste(image, (index % 4 * 320, index // 4 * 180))
    storyboard.save(root / f'storyboard-{scene}.png')
cards = ''.join(f'<article><h2>{name}</h2><img src="scene-{index + 26}.gif" alt="{name} synthetic audition"></article>' for index, name in enumerate(names))
band_review = ''
if (root / 'response-26-baseline.ppm').exists():
    bands = ['baseline', 'bass', 'mids', 'highs']
    comparison = Image.new('RGB', (1280, 1260), '#090d16')
    labels = ImageDraw.Draw(comparison)
    for row, name in enumerate(names):
        for column, band in enumerate(bands):
            x, y = column * 320, row * 210
            still = Image.open(root / f'response-{row + 26}-{band}.ppm').resize((320, 180))
            comparison.paste(still, (x, y + 30))
            labels.text((x + 8, y + 10), f'{name} / {band}', fill='white')
    comparison.save(root / 'band-response.png')
    band_review = '<h2>Isolated band response</h2><p>The camera, clock, palette, and exposure are fixed. Each column adds only one frequency band.</p><img src="band-response.png" alt="Six scenes responding separately to bass, mids, and highs"><p><a href="band-response.txt">Normalized spatial response measurements</a></p>'
color_review = ''
color_paths = sorted(root.glob('color-motion-*.ppm'))
if color_paths:
    frames = [Image.open(path).quantize(colors=128, method=Image.Quantize.FASTOCTREE) for path in color_paths]
    frames[0].save(root / 'color-motion.gif', save_all=True, append_images=frames[1:], duration=67, loop=0, optimize=False)
    comparison = Image.new('RGB', (1280, 1260), '#090d16')
    labels = ImageDraw.Draw(comparison)
    for row, name in enumerate(names):
        for column, label in enumerate(['Electric / 0', 'Electric / 0.33', 'Electric / 0.66', 'Sunset']):
            x, y = column * 320, row * 210
            still = Image.open(root / f'color-{row + 26}-{column}.ppm').resize((320, 180))
            comparison.paste(still, (x, y + 30))
            labels.text((x + 8, y + 10), f'{name} / {label}', fill='white')
    comparison.save(root / 'color-response.png')
    color_review = '<h2>Traveling color and palette transitions</h2><p>Geometry is held fixed in this clip so you can isolate color travel. The palette moves through Electric, Sunset, Neon, and Ocean.</p><img style="max-width:960px" src="color-motion.gif" alt="Traveling color with fixed geometry"><img src="color-response.png" alt="All six line worlds at different color phases and palettes"><p><a href="color-response.txt">Color response measurements</a></p>'
dissolve_review = ''
for start, end in [(0, 27), (27, 0), (8, 26), (26, 8), (25, 30), (30, 25)]:
    paths = sorted(root.glob(f'dissolve-{start}-{end}-[0-9][0-9][0-9].ppm'))
    if not paths:
        continue
    frames = [Image.open(path).quantize(colors=128, method=Image.Quantize.FASTOCTREE) for path in paths]
    filename = f'dissolve-{start}-{end}.gif'
    frames[0].save(root / filename, save_all=True, append_images=frames[1:], duration=[500] + [48] * (len(frames) - 2) + [500], loop=0, optimize=False)
    dissolve_review += f'<article><h2>Scene {start} → {end}</h2><img src="{filename}" alt="Isolated completed scene dissolve"></article>'
if dissolve_review:
    dissolve_review = '<h2>2D ↔ 3D dissolves</h2><p>Time and audio are fixed here to make transition continuity visible. The incoming palette and pattern seed differ from the outgoing scene.</p><main>' + dissolve_review + '</main><p><a href="transition-response.txt">Linear-light endpoint and midpoint checks</a></p>'
(root / 'review.html').write_text('''<!doctype html><html lang="en"><meta charset="utf-8"><meta name="viewport" content="width=device-width"><title>PulseBridge 0.1.6 · synthetic visual audition</title><style>body{margin:0;padding:32px;background:#090d16;color:#dce8f4;font:16px system-ui}h1{font-size:28px}p{max-width:850px;color:#9dafbd;line-height:1.6}main{display:grid;grid-template-columns:repeat(auto-fit,minmax(360px,1fr));gap:24px}article{background:#111925;border-radius:12px;overflow:hidden}h2{font-size:18px;padding:0 18px}img{display:block;width:100%}a{color:#83dfed}</style><h1>PulseBridge · reactive line worlds</h1><p>Development audition using the production native shaders and synthetic inputs. No audio was captured. The six scene loops use Electric; the separate color audition also demonstrates palette changes. White flashing is off throughout. Each 12-second loop moves through quiet, groove, build, impact, and breakdown; seconds 4–8 deliberately use the lowest shader detail.</p><main>''' + cards + '</main>' + band_review + color_review + dissolve_review + '''<h2>Ribbon Reactor → Shockwave Tunnel</h2><img style="max-width:960px" src="transition.gif" alt="Synthetic crossfade"><h2>Original Warp Spiral → Ribbon Reactor</h2><img style="max-width:960px" src="mixed-transition.gif" alt="Original pattern to line world crossfade"><p><a href="timings.txt">Native GPU frame timings</a></p></html>''')
print(root / 'review.html')
