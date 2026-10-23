"""Malowane ilustracje kart i portrety postaci z obrazów w domenie publicznej (Wikimedia Commons).

Każdy wpis w art/paintings/sources.json wskazuje plik z Commons, malarza, licencję, kadr (ułamki
szerokości i wysokości źródła) i opcjonalne poprawki korekty barwnej. Skrypt pobiera źródło
(pamięć podręczna w target/paintings-cache/), kadruje je do 8:5 (karty) albo 3:2 (portrety),
nadaje wspólną tonację palety gry i zapisuje pliki AVIF w art/paintings/{cards,avatars,backdrops,scenes}/
(tła widoków: 16:9, przyciemnione i lekko rozmyte, bo leżą pod treścią; sceny miejsc: 16:9 i ostre,
bo są banerami ekranów; wpis może podać własny rozmiar "size" w tych samych proporcjach).
Obraz miasta (wpis z "places") bierze rozmiar i obrysy budynków z pliku obok sources.json: budynki do
kliknięcia zostają jasne i ostre, a reszta miasta jest przygaszona, szarawa i rozmyta; "max_kib"
zastępuje domyślny limit pliku.
Na końcu odświeża stronę przeglądu art/paintings-review.html.
Ilustracja karty musi być czytelna w rozmiarze karty w grze (ADR-021): duże kształty mają przewagę
nad drobnymi szczegółami (miara czytelności co najmniej 3). Skrypt kończy się błędem i wymienia
karty poniżej progu; trzeba wtedy wybrać ciaśniejszy kadr albo inny obraz.

Użycie: python scripts/generate-paintings.py [klucz ...]
        (bez kluczy: wszystkie wpisy; np. card.blight avatar.ash_widow)
        python scripts/generate-paintings.py --check   (czytelność wszystkich kart, bez pobierania)
Wymaga Pillow >= 11.3 (AVIF) i numpy.
"""

import hashlib
import html
import json
import re
import sys
import time
import urllib.parse
import urllib.request
from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw, ImageEnhance, ImageFilter

ROOT = Path(__file__).resolve().parent.parent
SOURCES = ROOT / "art" / "paintings" / "sources.json"
OUTPUT = ROOT / "art" / "paintings"
CACHE = ROOT / "target" / "paintings-cache"
API = "https://commons.wikimedia.org/w/api.php"
USER_AGENT = "SpellgambitArtPipeline/1.0 (https://github.com/Esakis/Cellmons)"

CARD_SIZE = (512, 320)
PORTRAIT_SIZE = (576, 384)
BACKDROP_SIZE = (1280, 720)
SCENE_SIZE = (960, 540)
QUALITY = 45
MIN_QUALITY = 30
MAX_BYTES = 30 * 1024  # budget in scripts/check-web-budget.mjs: 32 KiB per file
LUMA = np.array([0.2126, 0.7152, 0.0722], dtype=np.float32)
MIN_READABILITY = 3.0  # below it a card's picture reads as noise at its size in the game
SHADOW = np.array([34, 24, 18], dtype=np.float32) / 255  # ciepły ciemny brąz
HIGHLIGHT = np.array([246, 232, 204], dtype=np.float32) / 255  # pergamin


def request(url: str) -> bytes:
    for attempt in range(4):
        try:
            req = urllib.request.Request(url, headers={"User-Agent": USER_AGENT})
            with urllib.request.urlopen(req, timeout=60) as response:
                return response.read()
        except OSError:
            if attempt == 3:
                raise
            time.sleep(3 + attempt * 5)
    raise AssertionError("unreachable")


def thumbnail_url(title: str, width: int) -> str:
    query = urllib.parse.urlencode({
        "action": "query", "format": "json", "formatversion": "2", "titles": title,
        "prop": "imageinfo", "iiprop": "url", "iiurlwidth": width,
    })
    info = json.loads(request(f"{API}?{query}"))["query"]["pages"][0]["imageinfo"][0]
    return info.get("thumburl") or info["url"]


def source(title: str, width: int) -> Image.Image:
    url = thumbnail_url(title, width)
    path = CACHE / hashlib.sha1(url.encode()).hexdigest()
    if not path.exists():
        CACHE.mkdir(parents=True, exist_ok=True)
        path.write_bytes(request(url))
    return Image.open(path).convert("RGB")


def crop(image: Image.Image, box: list[float], aspect: float) -> Image.Image:
    """Largest crop with the given aspect centred on the box; the box width wins unless too tall."""
    width, height = image.size
    x0, y0, x1, y1 = box
    centre_x, centre_y = (x0 + x1) / 2 * width, (y0 + y1) / 2 * height
    w = (x1 - x0) * width
    h = w / aspect
    if h > (y1 - y0) * height * 1.02:
        h = (y1 - y0) * height
        w = h * aspect
    left = min(max(centre_x - w / 2, 0), width - w)
    top = min(max(centre_y - h / 2, 0), height - h)
    return image.crop((round(left), round(top), round(left + w), round(top + h)))


def grade(image: Image.Image, target_mean=0.36, saturation=0.82, vignette=0.42, ceiling=1.0) -> Image.Image:
    """Common tonality: levels, mid-tone gamma, softer colour, warm split tone and a vignette."""
    a = np.asarray(image, dtype=np.float32) / 255
    lum = a @ LUMA
    low, high = np.percentile(lum, [0.4, 99.6])
    a = np.clip((a - low) / max(high - low, 1e-3), 0, 1)
    lum = a @ LUMA
    gamma = np.clip(np.log(target_mean) / np.log(max(float(lum.mean()), 1e-3)), 0.6, 1.6)
    a = np.power(a, gamma)
    lum = a @ LUMA
    a = lum[..., None] + (a - lum[..., None]) * saturation
    t = np.clip(lum, 0, 1)[..., None]
    tone = SHADOW * (1 - t) + HIGHLIGHT * t
    tone_mean = tone.mean(axis=-1, keepdims=True)
    a = a * 0.86 + tone * lum[..., None] * 0.14 / np.maximum(tone_mean, 1e-3) * tone.mean()
    a = np.clip(a, 0, 1)
    a = a + 0.10 * (a - 0.5) * (1 - np.abs(2 * a - 1))
    h, w = lum.shape
    yy, xx = np.mgrid[0:h, 0:w].astype(np.float32)
    distance = np.sqrt(((xx - w / 2) / (w / 2)) ** 2 + ((yy - h / 2) / (h / 2)) ** 2) / np.sqrt(2)
    a = a * (1 - vignette * np.clip(distance - 0.25, 0, 1) ** 1.6)[..., None]
    # Engravings on white paper: the brightest tone becomes parchment instead of paper white.
    a = a * ceiling
    return Image.fromarray((np.clip(a, 0, 1) * 255 + 0.5).astype(np.uint8))


def spotlight(image: Image.Image, places: list) -> Image.Image:
    """Clickable buildings keep their light; the rest of the town recedes (darker, greyer, softer)."""
    w, h = image.size
    mask = Image.new("L", image.size, 0)
    draw = ImageDraw.Draw(mask)
    for place in places:
        draw.polygon([(x * w / 100, y * h / 100) for x, y in place["shape"]], fill=255)
    mask = mask.filter(ImageFilter.MaxFilter(round(w * 0.004) | 1))
    mask = mask.filter(ImageFilter.GaussianBlur(w * 0.004))
    back = image.filter(ImageFilter.GaussianBlur(w * 0.0013))
    back = ImageEnhance.Brightness(ImageEnhance.Color(back).enhance(0.6)).enhance(0.72)
    back = Image.blend(back, Image.new("RGB", image.size, (29, 21, 14)), 0.3)
    return Image.composite(ImageEnhance.Brightness(image).enhance(1.06), back, mask)


def folder(kind: str) -> str:
    return {"avatar": "avatars", "backdrop": "backdrops", "scene": "scenes"}.get(kind, "cards")


def render(key: str, entry: dict) -> Path:
    kind, name = key.split(".", 1)
    size = {"avatar": PORTRAIT_SIZE, "backdrop": BACKDROP_SIZE, "scene": SCENE_SIZE}.get(kind, CARD_SIZE)
    # Dense miniatures and engravings stay within the file budget at a smaller size.
    size = tuple(entry.get("size", size))
    town = json.loads((OUTPUT / entry["places"]).read_text("utf-8")) if "places" in entry else None
    if town:
        size = tuple(town["size"])
    image = crop(source(entry["file"], entry.get("width", 1920)), entry["box"], size[0] / size[1])
    image = image.resize(size, Image.LANCZOS)
    if kind == "backdrop":
        # Backdrops sit under text: soft focus reads as depth and compresses far better.
        image = image.filter(ImageFilter.GaussianBlur(1.6))
    else:
        image = image.filter(ImageFilter.UnsharpMask(radius=1.1, percent=55, threshold=2))
    image = grade(image, **entry.get("grade", {}))
    if town:
        image = spotlight(image, town["places"])
    path = OUTPUT / folder(kind) / f"{name}.avif"
    path.parent.mkdir(parents=True, exist_ok=True)
    # Fine engravings compress worse than paintings: lower the quality until the file fits.
    limit = entry.get("max_kib", MAX_BYTES / 1024) * 1024
    quality = QUALITY
    image.save(path, "AVIF", quality=quality, speed=4)
    while path.stat().st_size > limit and quality > MIN_QUALITY:
        quality -= 3
        image.save(path, "AVIF", quality=quality, speed=4)
    return path


def review_page(entries: dict) -> None:
    """Static gallery for the owner's review: picture, Polish name, painter and source."""
    texts = (ROOT / "content" / "i18n" / "pl.ftl").read_text("utf-8")

    def name(key: str) -> str:
        found = re.search(rf"^{re.escape(key.replace('.', '-'))}-name\s*=\s*(.+)$", texts, re.M)
        return found.group(1).strip() if found else key

    figures = []
    for key, entry in entries.items():
        kind, file = key.split(".", 1)
        group = folder(kind)
        figures.append(
            f'<figure class="{group}"><img src="paintings/{group}/{file}.avif" alt="" loading="lazy">'
            f"<figcaption><b>{html.escape(name(key))}</b> <code>{html.escape(key)}</code><br>"
            f'{html.escape(entry["painter"])} · <a href="{html.escape(entry["page"])}">'
            f'{html.escape(entry["license"])}</a></figcaption></figure>'
        )
    generated = json.loads((OUTPUT / "generated-assets.json").read_text("utf-8"))
    for entry in generated["assets"]:
        figures.append(
            f'<figure><img src="paintings/{html.escape(entry["path"])}" alt="" loading="lazy">'
            f'<figcaption><b>{html.escape(entry["path"])}</b><br>'
            '<a href="paintings/generated-assets.json">AI · image_gen · prompt / source</a>'
            '</figcaption></figure>'
        )
    page = (
        '<!doctype html>\n<html lang="pl"><head><meta charset="utf-8">'
        "<title>Obrazy kart i postaci</title><style>"
        "body{margin:0;padding:16px;background:#0a090a;color:#dbd0bc;font:13px/1.4 system-ui,sans-serif}"
        "h1{font-weight:600;color:#e3cf9d}main{display:grid;gap:12px;"
        "grid-template-columns:repeat(auto-fill,minmax(256px,1fr))}"
        "figure{margin:0;background:#151315;border:1px solid #3a3027}"
        "img{display:block;width:100%;aspect-ratio:8/5;object-fit:cover}"
        ".avatars img{aspect-ratio:3/2}.backdrops img,.scenes img{aspect-ratio:16/9}"
        "figcaption{padding:6px 8px}code{color:#948876}a{color:#cbae78}"
        "</style></head><body><h1>Obrazy kart i postaci (ADR-021)</h1>"
        "<p>Źródła: art/paintings/sources.json. Strona powstaje w scripts/generate-paintings.py.</p>"
        f"<main>{''.join(figures)}</main></body></html>\n"
    )
    (ROOT / "art" / "paintings-review.html").write_text(page, "utf-8", newline="\n")


def blur(a: np.ndarray, sigma: float) -> np.ndarray:
    radius = max(1, int(3 * sigma))
    x = np.arange(-radius, radius + 1)
    kernel = np.exp(-(x**2) / (2 * sigma**2))
    kernel /= kernel.sum()
    for axis in (0, 1):
        pad = [(0, 0), (0, 0)]
        pad[axis] = (radius, radius)
        padded = np.pad(a, pad, mode="reflect")
        a = np.lib.stride_tricks.sliding_window_view(padded, 2 * radius + 1, axis=axis) @ kernel
    return a


def readability(path: Path) -> float:
    """Contrast of big shapes against the fine detail, at the card's size in the game (176 × 110)."""
    small = Image.open(path).convert("RGB").resize((176, 110), Image.LANCZOS)
    lum = (np.asarray(small, dtype=np.float32) / 255) @ LUMA
    return float(blur(lum, 4).std() / np.abs(lum - blur(lum, 1)).mean())


def main() -> None:
    sys.stdout.reconfigure(encoding="utf-8")
    if "AVIF" not in Image.registered_extensions().values():
        sys.exit("Pillow bez obsługi AVIF: zainstaluj Pillow >= 11.3.")
    entries = json.loads(SOURCES.read_text("utf-8"))
    busy = []
    if sys.argv[1:] == ["--check"]:
        paths = sorted((OUTPUT / "cards").glob("*.avif"))
        busy = [f"card.{path.stem}" for path in paths if readability(path) < MIN_READABILITY]
        print(f"Sprawdzono {len(paths)} kart.")
    else:
        keys = sys.argv[1:] or list(entries)
        unknown = [key for key in keys if key not in entries]
        if unknown:
            sys.exit(f"Nieznane klucze: {', '.join(unknown)}")
        total = 0
        for key in keys:
            kind, name = key.split(".", 1)
            target = OUTPUT / folder(kind) / f"{name}.avif"
            previous = target.read_bytes() if target.exists() else None
            path = render(key, entries[key])
            total += path.stat().st_size
            score = readability(path) if kind == "card" else None
            if score is not None and score < MIN_READABILITY:
                busy.append(key)
                # A rejected picture never replaces the card's previous one.
                if previous is None:
                    path.unlink()
                else:
                    path.write_bytes(previous)
            note = f"  czytelność {score:4.1f}" if score is not None else ""
            print(f"{key:30} {path.stat().st_size / 1024:5.1f} KiB{note}")
        print(f"Razem {len(keys)} plików, {total / 1024:.0f} KiB")
        review_page(entries)
    if busy:
        sys.exit(
            f"Za dużo drobnych szczegółów w rozmiarze karty (czytelność < {MIN_READABILITY}): "
            f"{', '.join(busy)}. Wybierz ciaśniejszy kadr albo inny obraz."
        )


if __name__ == "__main__":
    main()
