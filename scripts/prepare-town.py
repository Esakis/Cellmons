"""Wersje mapy miasta (lobby) z obrazów wygenerowanych przez AI.

Wariant `nazwa` czyta art/paintings/generated/nazwa.png i układ art/paintings/nazwa.json
(rozmiar wyniku, obrysy budynków i punkty tabliczek w procentach obrazu). Obraz jest skalowany
do rozmiaru z układu, budynki zostają jasne, reszta miasta przygasa (jak w generate-paintings.py),
a wynik trafia do art/paintings/scenes/nazwa.avif w limicie 64 KB (scripts/check-web-budget.mjs).
Klient (Town.tsx) sam wybiera wariant najbliższy kształtem dostępnemu miejscu.

Użycie: python scripts/prepare-town.py town-portrait town-wide
Wymaga Pillow >= 11.3 (AVIF) i numpy.
"""

import importlib.util
import json
import sys
from pathlib import Path

from PIL import Image, ImageFilter

ROOT = Path(__file__).resolve().parent.parent
PAINTINGS = ROOT / "art" / "paintings"
LIMIT = 63 * 1024
spec = importlib.util.spec_from_file_location("paintings", ROOT / "scripts" / "generate-paintings.py")
paintings = importlib.util.module_from_spec(spec)
spec.loader.exec_module(paintings)


def prepare(name: str) -> Path:
    layout = json.loads((PAINTINGS / f"{name}.json").read_text("utf-8"))
    size = tuple(layout["size"])
    image = Image.open(PAINTINGS / "generated" / f"{name}.png").convert("RGB")
    ratio = size[0] / size[1]
    w, h = image.size
    # Trim a generator's slightly different proportions from the centre, never stretch.
    if abs(w / h - ratio) > 0.01:
        if w / h > ratio:
            cut = round(w - h * ratio)
            image = image.crop((cut // 2, 0, w - (cut - cut // 2), h))
        else:
            cut = round(h - w / ratio)
            image = image.crop((0, cut // 2, w, h - (cut - cut // 2)))
    image = image.resize(size, Image.LANCZOS)
    image = image.filter(ImageFilter.UnsharpMask(radius=1.1, percent=45, threshold=2))
    image = paintings.spotlight(image, layout["places"])
    path = PAINTINGS / "scenes" / f"{name}.avif"
    quality = 50
    image.save(path, "AVIF", quality=quality, speed=4)
    while path.stat().st_size > LIMIT and quality > 20:
        quality -= 3
        image.save(path, "AVIF", quality=quality, speed=4)
    print(f"{path.relative_to(ROOT)}: {size[0]}×{size[1]}, {path.stat().st_size // 1024} KiB, q={quality}")
    return path


if __name__ == "__main__":
    for variant in sys.argv[1:] or ["town-portrait", "town-wide"]:
        prepare(variant)
