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
names = ['Magnetic Swarm', 'Liquid Relic', 'Impossible Architecture', 'Aurora Veil', 'Kinetic Sculpture', 'Topographic Ocean']
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
(root / 'review.html').write_text('''<!doctype html><html lang="en"><meta charset="utf-8"><meta name="viewport" content="width=device-width"><title>PulseBridge 0.1.3 · synthetic visual audition</title><style>body{margin:0;padding:32px;background:#090d16;color:#dce8f4;font:16px system-ui}h1{font-size:28px}p{max-width:850px;color:#9dafbd;line-height:1.6}main{display:grid;grid-template-columns:repeat(auto-fit,minmax(360px,1fr));gap:24px}article{background:#111925;border-radius:12px;overflow:hidden}h2{font-size:18px;padding:0 18px}img{display:block;width:100%}a{color:#83dfed}</style><h1>PulseBridge · six new visual worlds</h1><p>Development audition using the production native shaders and synthetic inputs. No audio was captured. Every scene uses the Electric palette with white flashing off. Each 12-second loop moves through quiet, groove, build, impact, and breakdown; seconds 4–8 deliberately use the lowest shader detail.</p><main>''' + cards + '''</main><h2>Liquid Relic → Impossible Architecture</h2><img style="max-width:960px" src="transition.gif" alt="Synthetic crossfade"><p><a href="timings.txt">Native GPU frame timings</a></p></html>''')
print(root / 'review.html')
