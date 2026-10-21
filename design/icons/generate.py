#!/usr/bin/env python3
"""Regenerate EasyFlowHub app icons from the chosen concept-2 (glass flow) base.

Reads a high-resolution base render and downscales it (LANCZOS) into every size
the Tauri config, Windows Store assets and the multi-frame ICO/ICNS need. Run
from the repo so relative paths resolve; the base PNG must already exist.
"""
import os
from PIL import Image

HERE = os.path.dirname(os.path.abspath(__file__))
ICONS = os.path.normpath(os.path.join(HERE, '..', '..', 'easyflowhub-app', 'src-tauri', 'icons'))
BASE = os.path.join(HERE, 'preview', 'concept2-base-1024.png')

# Every standalone PNG the bundle / Store assets reference, keyed by filename.
PNG_SIZES = {
    '32x32.png': 32,
    '128x128.png': 128,
    '128x128@2x.png': 256,
    'icon.png': 512,
    'Square30x30Logo.png': 30,
    'Square44x44Logo.png': 44,
    'Square71x71Logo.png': 71,
    'Square89x89Logo.png': 89,
    'Square107x107Logo.png': 107,
    'Square142x142Logo.png': 142,
    'Square150x150Logo.png': 150,
    'Square284x284Logo.png': 284,
    'Square310x310Logo.png': 310,
    'StoreLogo.png': 50,
}

ICO_SIZES = [16, 32, 64, 128, 256]
ICNS_SIZES = [16, 32, 64, 128, 256, 512]


def scaled(base: Image.Image, size: int) -> Image.Image:
    return base.resize((size, size), Image.LANCZOS)


def main() -> None:
    if not os.path.exists(BASE):
        raise SystemExit(f'base render missing: {BASE}')
    base = Image.open(BASE).convert('RGBA')

    for name, size in PNG_SIZES.items():
        out = os.path.join(ICONS, name)
        scaled(base, size).save(out, 'PNG')
        print(f'png  {name:26} {size:>3}px')

    # Multi-frame ICO. Pillow writes every size passed via append_images.
    ico_path = os.path.join(ICONS, 'icon.ico')
    frames = [scaled(base, s) for s in ICO_SIZES]
    frames[-1].save(ico_path, 'ICO', sizes=[(s, s) for s in ICO_SIZES], append_images=frames[:-1])
    print(f'ico  icon.ico                   {ICO_SIZES}')

    # ICNS for the (unused on Windows but config-referenced) macOS bundle.
    icns_path = os.path.join(ICONS, 'icon.icns')
    icns_frames = [scaled(base, s) for s in ICNS_SIZES]
    icns_frames[-1].save(icns_path, 'ICNS', append_images=icns_frames[:-1])
    print(f'icns icon.icns                  {ICNS_SIZES}')

    # Keep the design source next to the generated assets for traceability.
    design_svg = os.path.join(HERE, 'concept-2-glass-flow.svg')
    if os.path.exists(design_svg):
        with open(design_svg, 'r', encoding='utf-8') as fh:
            svg = fh.read()
        with open(os.path.join(ICONS, 'icon.svg'), 'w', encoding='utf-8') as fh:
            fh.write(svg)
        print('svg  icon.svg                   (design source)')


if __name__ == '__main__':
    main()
