# Rejestr pochodzenia grafik

Rejestr wymagany przez `docs/12-bezpieczenstwo-i-prawo.md`, rozdz. 8: autor lub narzędzie, data, opis zamówienia, zatwierdzający i licencja każdej grafiki. Wkład właściciela (opis, wybór wariantu, poprawki) dokumentujemy w kolumnie „Zamówienie i uwagi”, bo od niego zależy ochrona utworów powstałych z udziałem AI.

Wszystkie grafiki poniżej są kodem SVG napisanym od zera, bez wzorowania się na cudzych grafikach, w tym na grafikach z oryginalnego Cellmons.

| Plik | Autor / narzędzie | Data | Zamówienie i uwagi | Zatwierdził | Licencja |
|---|---|---|---|---|---|
| `icons/school-*.svg` (6) | asystent AI Claude (Claude Code) | 2026-10-15 | F3.6: ikony 6 szkół według przewodnika stylu (siatka 24, linia 1,75, `currentColor`) | właściciel projektu, 2026-10-15 | własność projektu |
| `icons/attack-*.svg` (2) | asystent AI Claude (Claude Code) | 2026-10-15 | F3.6: ikony typów ataku F i M | właściciel projektu, 2026-10-15 | własność projektu |
| `icons/stat-*.svg` (7) | asystent AI Claude (Claude Code) | 2026-10-15 | F3.6: ikony statystyk AF, OF, AM, OM, Ż, W, K | właściciel projektu, 2026-10-15 | własność projektu |
| `frames/frame-*.svg` (3) | asystent AI Claude (Claude Code) | 2026-10-15 | F3.6: ramki kart brązowej, srebrnej i złotej, układ 250 × 350 | właściciel projektu, 2026-10-15 | własność projektu |
| `frames/card-layout.svg` | asystent AI Claude (Claude Code) | 2026-10-15 | F3.6: schemat stref karty do przewodnika stylu | właściciel projektu, 2026-10-15 | własność projektu |
| `avatars/*.svg` (6) | asystent AI Claude (Claude Code) | 2026-10-15 | F3.6: geometryczne portrety 6 postaci startowych (kolor i atrybut szkoły); poprawka: miecze Wędrowca Wydm z rękojeściami nad ramionami | właściciel projektu, 2026-10-15 | własność projektu |
| `cards/*.svg` (72) | asystent AI Claude (Claude Code) | 2026-10-15 | ilustracje wszystkich kart (6 szkół) według polecenia właściciela „dodaj grafiki do kart”, w ogólnym klimacie kolekcjonerskich gier karcianych; poprawka na prośbę właściciela („bardziej wypasione”): tła scen szkół, drobinki, ozdobna ramka; rysowane od zera, bez wzorowania się na konkretnych cudzych grafikach | oczekuje na właściciela | własność projektu |
| `avatars/*.svg` (6), wersja 2 | asystent AI Claude (Claude Code) | 2026-10-15 | nowe portrety klas (Wojownik, Kapłan, Druid, Mag, Nekromantka, Runotechnik) jako ilustrowane popiersia z tłem sceny, po zmianie nazw postaci na nazwy klas (decyzja właściciela) i prośbie o bogatsze grafiki; zastępują portrety geometryczne z F3.6 | oczekuje na właściciela | własność projektu |
| `gallery.html` | asystent AI Claude (Claude Code) | 2026-10-15 | strona do przeglądu grafik | nie dotyczy (narzędzie) | własność projektu |
| `../web/public/icon.svg`, `icon-192.png`, `icon-512.png` | Codex, techniczny eksport SVG/PNG | 2026-10-15 | F5: zatwierdzony symbol `icons/school-arcane.svg` bez zmiany geometrii, przeskalowany na tło z palety projektu; PNG renderowane z SVG | symbol zatwierdzony w F3.6; nie jest to nowy projekt znaku marki | własność projektu |

| `cards/{tactical_reserve,oath_of_steel,shared_vigil,dawn_reprieve,cinder_volley,storm_braid,measured_thought,still_hour,ash_tether,grave_bloom,brass_casing,overdrive_valve}.svg` (12) | Codex, autorskie rysunki SVG zapisane w `scripts/generate-forge-art.py` | 2026-10-15 | Pierwsza paczka beta: osobne kompozycje zasobów, osłon, czasu i magii szkół; bez cudzych materiałów i bez edycji wcześniejszych grafik | oczekuje na właściciela | materiał projektu, do przeglądu |

| `avatars/{cinder_knight,copper_distiller,ruin_keeper}.svg` (3) | Codex, autorskie rysunki SVG w `scripts/generate-junkyard-art.py` | 2026-10-15 | Klasy wysypiska: rycerz z pancerzem, destylator z aparaturą oraz opiekun ruin z laską; bez cudzych materiałów | oczekuje na właściciela | materiał projektu, do przeglądu |

Optymalizacja techniczna F9 (2026-10-15): ikony PWA PNG zakodowano ponownie bezstratnie narzędziem oxipng 10.2.1 (`-o max --strip safe`). 16619 → 7629 bajtów; wymiary i SHA-256 zdekodowanych pikseli RGBA obu plików pozostały identyczne. Geometria, kolory i zatwierdzony projekt symbolu nie zmieniły się.

| `cards/shield_line.svg` … `cards/reset_lever.svg` (48 kart drugiej paczki Forge; katalog `forge-beta-review.html`) | Codex, własne kompozycje SVG i geometria zapisana w `scripts/generate-beta-art.py`, bez materiałów zewnętrznych | 2026-10-15 | F9.5: metaliczne przedmioty i symbole szkół na tle palety projektu; każdy plik ma osobny rysunek | oczekuje na odbiór właściciela | własność projektu |

| `avatars/storm_surveyor.svg` … `avatars/dawn_lancer.svg` (10 nowych klas; katalog `beta-portraits-review.html`) | Codex, własne sylwetki i wyposażenie zapisane w `scripts/generate-beta-portraits.py`, bez materiałów zewnętrznych | 2026-10-15 | F9.5: wspólna rama i materiały, osobne rysunki klas | oczekuje na odbiór właściciela | własność projektu |
