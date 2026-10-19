# Przewodnik stylu grafiki

Źródła grafik gry (ADR-016 i ADR-021, `docs/03-tresc-gry.md`, rozdz. 9). Każdy plik ma wpis w [rejestrze pochodzenia](PROVENANCE.md). Do przeglądu służą [galeria SVG](gallery.html) i [przegląd obrazów](paintings-review.html): otwórz je przez lokalny serwer, np. `python -m http.server` w katalogu `art/`.

**Status:** kierunek „dla dorosłych” (ADR-021) zatwierdzony przez właściciela 2026-10-15 po porównaniu zrzutów prototypu. Wcześniejsza wersja SVG z kroku F3.6 została zatwierdzona 2026-10-15 i zostaje dla ramek, interfejsu i kart bez obrazu.

## 1. Kierunek

- Ciemne fantasy dla dorosłych: czernione żelazo, postarzany mosiądz i bordo, drobne ziarno zamiast płaskich gradientów.
- Ilustracje kart i portrety postaci to obrazy dawnych mistrzów w domenie publicznej, kadrowane i przepuszczone przez wspólną korektę barwną, żeby różni malarze tworzyli jedną całość (rozdz. 5a i 6).
- Ikony to pełne sylwetki z game-icons.net: na ciemnym tle w kolorze mosiądzu, na emaliowanych monetach ataku i obrony w kolorze kości.
- Kolor niesie znaczenie, a nie dekorację: życie (bordo), typ ataku (fizyczny: bordo, magiczny: granat), koszty K i W, kolory szkół.
- Nie wzorujemy się na grafikach z oryginalnego Cellmons ani na innych chronionych grafikach (`docs/12-bezpieczenstwo-i-prawo.md`, rozdz. 8).

## 2. Paleta

Kolory szkół są też w `content/schools.toml` (pola `color` i `color_dark`); przy zmianie trzeba poprawić oba miejsca.

| Szkoła | Motyw jasny | Motyw ciemny |
|---|---|---|
| Walka | `#b23a2e` | `#e5735f` |
| Kapłaństwo | `#b8860b` | `#e6c25a` |
| Elementalizm | `#2e7d32` | `#6fbf73` |
| Magia | `#3949ab` | `#8c9eff` |
| Nekromancja | `#6a1b9a` | `#b77fd1` |
| Technologia | `#00838f` | `#4dd0e1` |

Interfejs w motywie ciemnym (`web/src/styles/tokens.css`, ekran walki: `Training.module.css`):

| Rola | Wartość |
|---|---|
| Tło strony | `#0a090a` z ziarnem (`--grain`) |
| Panele | `#1b181a` → `#100f11` |
| Mosiądz: linie, ramki | `#8c7247`, `#b3925a` |
| Mosiądz: nagłówki | `#e3cf9d` |
| Tekst / tekst pomocniczy | `#dbd0bc` / `#948876` |
| Przycisk główny (bordo) | `#8c2b22` → `#3b0e0a`, tekst `#f2e3c3` |
| Moneta ataku fizycznego | emalia `#b8493c` → `#2c0806` w mosiężnej oprawie |
| Moneta ataku magicznego | emalia `#5f80b8` → `#0c1528` w mosiężnej oprawie |
| Ikony na ciemnym tle | filtr `--icon-filter` (mosiądz `#d4bb86`) |
| Ikony na emalii | filtr `--icon-filter-bone` (kość `#dccfb6`) |

Rzadkości kart (gradient metalu z odblaskiem):

| Rzadkość | Jasny | Środek | Ciemny |
|---|---|---|---|
| Brązowa | `#d6a06a` | `#8a5428` | `#3f230d` |
| Srebrna | `#eef1f4` | `#8f98a2` | `#3c4249` |
| Złota | `#f6e3a1` | `#b48a2c` | `#5a4210` |

## 3. Typografia

- Nagłówki i nazwy: **Cinzel** (SIL OFL 1.1), interfejs i teksty kart: **Inter** (SIL OFL 1.1); pliki WOFF2 w `web/src/styles/fonts/`.
- Nazwa karty: 13–15 px, grubość 600–700. Tekst efektu: 11–13 px, grubość 400.
- Liczby na monetach: Cinzel, grubość 700.

## 4. Ikony (`icons/`)

- Źródło: [game-icons.net](https://game-icons.net) (repozytorium `game-icons/icons`), licencja CC BY 3.0. Autorzy użytych ikon: Lorc, Delapouite, Sbed i Carl Olsen. Podpis jest w ustawieniach gry, a mapa plików w rejestrze pochodzenia.
- Pliki zachowują kształt oryginału (siatka 512 × 512, jedna ścieżka); usunięte jest tylko czarne tło. Kolor nadaje klient filtrem CSS. Ikony interfejsu (`nav-*`, `online/`) mają siatkę 128 × 128 z zaokrąglonymi współrzędnymi: są lżejsze o ok. 30%, a różnicy nie widać.
- Zestaw: 6 szkół (`school-*`), typy ataku F i M (`attack-*`), statystyki AF, OF, AM, OM, Ż, W i K (`stat-*`), nawigacja (`nav-*`: Trening, Lobby, Kolekcja, Sklep, Konto, Ustawienia).
- `online/`: ikony ekranów sieciowych, poza pakietem offline: zakładki lobby (`tab-*`; `tab-ranking` oznacza też RP), Dukaty, PD, poziom i nieobecność (AFK).
- Każda ikona ma `<title>` i `aria-label` z polską nazwą.

## 5. Karta (`frames/`)

Format 250 × 350 (proporcje 5 : 7). Układ pokazuje `frames/card-layout.svg`:

| Strefa | Położenie | Zawartość |
|---|---|---|
| Koszt K | lewy górny róg, koło Ø 34 | liczba K |
| Koszt W | pod kosztem K, koło Ø 26 | liczba W |
| Nazwa | pasek u góry, obok kosztów | nazwa karty w języku gracza |
| Ilustracja | okno 8 : 5 | obraz z `paintings/cards/`, SVG z `cards/` albo ikona szkoły |
| Wymagania | pasek pod ilustracją | ikony szkół z wymaganym poziomem |
| Tekst | pole pod wymaganiami | tekst efektu i słowa kluczowe |
| Rzadkość | ramka | kolor metalu ramki |

Ramki (`frame-bronze.svg`, `frame-silver.svg`, `frame-gold.svg`) nie zawierają tekstu: nazwę, koszty i opis wstawia klient z `content.json` i plików tekstów, więc ta sama ramka działa po polsku i po angielsku.

## 5a. Ilustracje kart (`paintings/cards/`, zapasowo `cards/`)

- Obrazy dawnych mistrzów z Wikimedia Commons, wyłącznie z licencją „Public domain” albo CC0. Nie używamy plików CC BY-SA ani zdjęć sal muzealnych z własną licencją. Unikamy aktów i drastycznych scen.
- Temat obrazu odpowiada nazwie albo działaniu karty (np. „Groza” — „Zmora” Füssliego, „Wyspa” Böcklina dla „Milczącego dworu”, kuźnie Wright of Derby dla Runotechniki).
- Kadr 8 : 5, plik 512 × 320 AVIF (jakość 45, a gdy plik przekracza 30 KB, np. przy drobnych rycinach, jakość spada co 3 do najwyżej 30), zwykle 10–25 KB. Wspólna korekta barwna: poziomy, gamma do wspólnej jasności, lekka desaturacja, ciepłe cienie i jasne tony pergaminu, delikatna krzywa S i winieta. W kliencie obraz wypełnia okno (`object-fit: cover`) i ma wewnętrzny cień jak płótno w ramie.
- Źródłem prawdy jest `paintings/sources.json`: plik w Commons, strona opisu, malarz, licencja, kadr (ułamki szerokości i wysokości źródła) i ewentualne poprawki korekty. `scripts/generate-paintings.py` odtwarza z niego pliki AVIF i stronę przeglądu.
- Karta bez obrazu pokazuje ilustrację SVG z `cards/`, a bez niej ikonę szkoły. Nazwy plików to klucze kart z `content/cards` bez prefiksu `card.`.

## 6. Portrety postaci (`paintings/avatars/`, zapasowo `avatars/`)

- Portrety i postacie z obrazów dawnych mistrzów dobrane do klasy (np. Wojownik — „Mężczyzna w zbroi” Rembrandta, Nekromantka — „Medea” Sandysa, Runotechnik — „Alchemik” Wright of Derby).
- Kadr 3 : 2, plik 576 × 384 AVIF; twarz w środkowej części kadru, bo klient wycina z niego kwadrat i koło (`object-fit: cover`).
- Drogi postaci (`*_path_0`, `*_path_1`) używają obrazu postaci bazowej. Postać bez obrazu pokazuje portret SVG z `avatars/`.

## 7. Pliki i optymalizacja

- `icons/` (z `icons/online/`), `frames/`, `avatars/`, `cards/`: źródła SVG; przed włączeniem do klienta przechodzą przez SVGO (`scripts/prepare-web.mjs`).
- `paintings/backdrops/`: tła widoków (16 : 9, rozmyte i przyciemnione; `Shell.tsx` wybiera tło według trasy, `default` dla pozostałych).
- `paintings/scenes/`: sceny miejsc (16 : 9, 960 × 540, ostre): baner miejsca, w którym jest gracz, tło strony w tym miejscu oraz banery ekranów (`home`, `collection`, `shop`, `clan`, `account`, `settings`, `tournament`).
- `paintings/scenes/town.avif` i `paintings/town.json`: mapa lobby jako jedno miasto (1920 × 1302, limit 64 KB). `town.json` trzyma rozmiar obrazu, obrysy budynków i punkty tabliczek w procentach obrazu. Na jego podstawie `generate-paintings.py` przygasza wszystko poza budynkami, a klient (`Town.tsx`) rysuje obrysy i tabliczki. Po przesunięciu budynku trzeba poprawić `town.json` i wygenerować obraz od nowa (`python scripts/generate-paintings.py scene.town`).
- `paintings/`: pliki AVIF i `sources.json`; `prepare-web.mjs` kopiuje je do `web/src/assets/paintings/` i pomija SVG o tej samej nazwie.
- Budżet (`scripts/check-web-budget.mjs`): obrazy ≤ 6 MB łącznie i ≤ 32 KB na plik; pozostałe budżety z `docs/13-wydajnosc.md` bez zmian.
- Nowy obraz: wybór pliku w Commons (licencja!) → wpis w `sources.json` → `python scripts/generate-paintings.py card.klucz` → przegląd → wpis w `PROVENANCE.md`.
- Ilustracja karty musi być czytelna w rozmiarze karty: jeden temat, duże kształty, bez drobnej faktury na całym kadrze (ryciny, rękopisy, tłumy). Skrypt podaje czytelność każdej karty i kończy się błędem poniżej 3; `python scripts/generate-paintings.py --check` sprawdza wszystkie karty bez pobierania źródeł.
