# Mapa miasta: wersja pionowa i szeroka

Obecna mapa (`paintings/scenes/town.avif`) jest kwadratowa, więc na telefonie zostaje pusty pas pod nią, a w przeglądarce po bokach. Klient (`web/src/features/lobby/Town.tsx`) wybiera teraz wariant najbliższy kształtem wolnemu miejscu:

| Wariant | Gdzie | Plik AI | Wynik w grze |
|---|---|---|---|
| `town` | zostaje jako zapas, np. tablet | `generated/town.png` | 1024 × 1024 |
| `town-portrait` | telefon w pionie | `generated/town-portrait.png` (9:16, np. 1024 × 1792) | 864 × 1536 |
| `town-wide` | przeglądarka na komputerze | `generated/town-wide.png` (16:9, np. 1792 × 1024) | 1536 × 864 |

Dopóki nie ma pliku wariantu i jego układu (`paintings/town-portrait.json`, `paintings/town-wide.json`), gra pokazuje kwadratową mapę jak dotąd. Obie wersje są w grze od 2026-10-28.

## Jak dodać wariant

1. Wygeneruj obraz z promptu poniżej, w podanych proporcjach. Budynki muszą stać mniej więcej w podanych miejscach.
2. Zapisz go jako `art/paintings/generated/town-portrait.png` albo `town-wide.png`.
3. Obrysuj budynki w `art/paintings/town-portrait.json` (format jak `town.json`: `size`, a dla każdego miejsca `at` – punkt tabliczki i `shape` – obrys, w procentach obrazu). Na szerokiej mapie tabliczka ma 17% szerokości (`lobby.css`), więc punkty `at` w jednym rzędzie leżą co najmniej 18% od siebie i 9% od krawędzi.
4. `python scripts/prepare-town.py town-portrait` tworzy `paintings/scenes/town-portrait.avif` w limicie 64 KB.
5. Dopisz wpis z promptem do `paintings/generated-assets.json`.

## Wspólna część promptu

> Use case: stylized-concept. Create a complete aerial painted fantasy town map background for a mobile dark fantasy card game. Production artwork only: NO text, labels, typography, numbers, UI, pins, frames, legend or watermarks. The entire town must fit within the image with all ten distinct destination landmarks visible simultaneously. Elevated near top-down isometric bird's-eye view, rich traditional oil/gouache painting, dark romantic medieval fantasy, aged limestone, slate roofs, bronze and warm amber window lights against desaturated forest greens, a little purple magic. Beautiful legible large buildings separated by winding cobbled roads, trees and small plazas; sparse tiny filler houses so landmarks dominate. Continuous believable terrain and connecting paths, not a tile grid. Make all ten buildings clearly different in silhouette and large enough for mobile use. No horizon or sky, no perspective foreground occlusion. Composition uses all edges with a narrow natural forest/stone perimeter, each destination well inside the image, with some quiet ground just below each landmark for code-rendered labels. This is an atmospheric hand-painted explorable town, not a schematic diagram. Opaque background.

## Pionowa (telefon) – dopisz do wspólnej części

> Tall PORTRAIT 9:16 image. Landmarks at these approximate centers in percent of image width/height: 1 slender scholar tower (24,8); 2 royal fortified castle (70,9); 3 broad library with columns and slate-blue roofs (26,25); 4 grand circular tournament colosseum with red banners (72,26); 5 smaller open oval combat arena with sandy floor (50,42); 6 ancient stone magical portal with restrained violet glow (24,57); 7 fortified guild hall with heraldic banner (76,57); 8 colorful cloth market stalls (50,71); 9 training courtyard with wooden sparring dummies (25,87); 10 merchant shop with gold lit windows (75,87). Two columns of landmarks with the arena and market in the middle, roads winding top to bottom.

## Szeroka (komputer) – dopisz do wspólnej części

> Wide LANDSCAPE 16:9 image. Landmarks at these approximate centers in percent of image width/height: top row – 1 slender scholar tower (10,24); 2 royal fortified castle (30,20); 3 grand circular tournament colosseum with red banners (52,22); 4 fortified guild hall with heraldic banner (72,24); 5 broad library with columns and slate-blue roofs (90,26). Bottom row – 6 training courtyard with wooden sparring dummies (10,70); 7 ancient stone magical portal with restrained violet glow (30,68); 8 smaller open oval combat arena with sandy floor (52,66); 9 colorful cloth market stalls (72,70); 10 merchant shop with gold lit windows (90,72). A cobbled road runs between the two rows.
