# Przewodnik stylu grafiki

Źródła grafik gry w SVG (ADR-016, `docs/03-tresc-gry.md`, rozdz. 9). Grafiki tworzy asystent AI element po elemencie, a właściciel projektu wybiera warianty i zatwierdza wersje końcowe. Każdy plik ma wpis w [rejestrze pochodzenia](PROVENANCE.md). Do przeglądu służy [galeria](gallery.html): otwórz ją przez lokalny serwer, np. `python -m http.server` w katalogu `art/`, bo strona wczytuje pliki SVG.

**Status:** wersja z kroku F3.6, zatwierdzona przez właściciela 2026-10-15.

## 1. Kierunek

- Motywem przewodnim są emblematy i sygile szkół.
- Portrety i ilustracje kart są ilustracjami wektorowymi w klimacie fantasy (gradienty, światło, tło sceny). Nie ma realistycznego malarstwa ani faktur.
- Ikony są proste i płaskie, bez cieni i faktur. Grafiki mają być czytelne od 32 px (ikony) do pełnej karty na ekranie telefonu.
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

Kolory wspólne:

| Rola | Wartość |
|---|---|
| Pergamin (tło karty i jasnego motywu) | `#f8f3e8` |
| Pole tekstu karty | `#fffdf8` |
| Tekst | `#2b2118` |
| Tekst pomocniczy | `#6b5d4f` |
| Tło ciemnego motywu | `#1d1a22` |
| Koszt K (koncentracja) | `#2f4f8f` |
| Koszt W (wytrzymałość) | `#2e7d4f` |

Rzadkości kart (gradient metalu od jasnego do ciemnego):

| Rzadkość | Jasny | Środek | Ciemny |
|---|---|---|---|
| Brązowa | `#e2a86a` | `#b8773d` | `#6f4420` |
| Srebrna | `#e6eaef` | `#a8b0ba` | `#5f6670` |
| Złota | `#f6dc7a` | `#c9a227` | `#7a5a00` |

## 3. Typografia

- Jedna czcionka zmienna z polskimi znakami (`latin-ext`): **Inter** (licencja SIL OFL 1.1), do interfejsu i tekstów kart. Plik WOFF2 dojdzie razem z klientem w fazie F5 (`docs/10-klient-web.md`, rozdz. 9). Do tego czasu i jako zapas: czcionki systemowe (`system-ui, sans-serif`).
- Nazwa karty: 13–15 px, grubość 700. Tekst efektu: 11–13 px, grubość 400, słowa kluczowe (np. „Umagicznienie”, „Interwencja”) pogrubione.
- Liczby kosztów na kartach: grubość 700, biały tekst na kolorze kosztu.

## 4. Ikony (`icons/`)

- Siatka 24 × 24, linia 1,75 px, zaokrąglone końce i narożniki, bez wypełnień (poza drobnymi akcentami).
- Kolor z `currentColor`, więc ikona przyjmuje kolor tekstu albo szkoły i działa w obu motywach.
- Zestaw: 6 szkół (`school-*`), typy ataku F i M (`attack-*`), statystyki AF, OF, AM, OM, Ż, W i K (`stat-*`).
- Każda ikona ma `<title>` i `aria-label` z polską nazwą; w kliencie trafią do jednego sprite'a SVG.

## 5. Karta (`frames/`)

Format 250 × 350 (proporcje 5 : 7). Układ pokazuje `frames/card-layout.svg`:

| Strefa | Położenie | Zawartość |
|---|---|---|
| Koszt K | lewy górny róg, koło Ø 34 | liczba K |
| Koszt W | pod kosztem K, koło Ø 26 | liczba W |
| Nazwa | pasek u góry, obok kosztów | nazwa karty w języku gracza |
| Ilustracja | okno 210 × 122 | ilustracja karty z `cards/` albo ikona szkoły |
| Wymagania | pasek pod ilustracją | ikony szkół z wymaganym poziomem |
| Tekst | pole 210 × 84 | tekst efektu i słowa kluczowe |
| Rzadkość | klejnot na dolnej krawędzi | kolor metalu ramki |

Ramki (`frame-bronze.svg`, `frame-silver.svg`, `frame-gold.svg`) nie zawierają tekstu: nazwę, koszty i opis wstawia klient z `content.json` i plików tekstów, więc ta sama ramka działa po polsku i po angielsku.

## 5a. Ilustracje kart (`cards/`)

- Scena 160 × 100 (proporcje 8 : 5): tło z gradientem w kolorach szkoły, promienie światła, tło sceny szkoły (arena, świątynia, wzgórza, gwiazdy, cmentarz, warsztat), drobinki, cień na ziemi, winieta i złota ozdobna ramka.
- Motyw na środku: przedmiot, istota lub zjawisko z działania karty. Kontur ciemny 1,2–1,8 px, połysk metalu i szkła gradientami.
- W odróżnieniu od ikon ilustracje mają gradienty i światło, ale nadal bez faktur i realistycznego malarstwa.
- Nazwy plików to klucze kart z `content/cards` bez prefiksu `card.`. Karta bez ilustracji pokazuje ikonę szkoły.
- W kliencie wszystkie ilustracje trafiają do jednego, leniwie ładowanego modułu (`web/src/app/cardArt.ts`), bo razem kompresują się kilka razy lepiej niż osobne pliki (budżet aplikacji offline).

## 6. Portrety postaci (`avatars/`)

- Koło 200 × 200 z obwódką w kolorze szkoły; tło sceny w gradiencie szkoły (promienie, gwiazdy, las, warsztat).
- Ilustrowane popiersie klasy (od 2026-10-15): twarz z oczami i cieniowaniem, strój i rekwizyt szkoły: hełm z pióropuszem i miecz (Wojownik), aureola i laska słońca (Kapłan), wieniec z poroża i liści (Druid), kapelusz z gwiazdami i kula światła (Mag), kaptur i czaszka (Nekromantka), gogle i koło zębate (Runotechnik).
- Nazwy plików to klucze postaci z `content/avatars` bez prefiksu `avatar.`.

## 7. Pliki i optymalizacja

- `icons/`, `frames/`, `avatars/`, `cards/`: źródła SVG pisane ręcznie, bez zbędnych atrybutów edytorów.
- Przed włączeniem do klienta (faza F5) pliki przechodzą przez SVGO; wersje zoptymalizowane trafiają do `web/src/assets/`, a źródła zostają tutaj.
- Nowa grafika: opis → 2–3 warianty → wybór właściciela → poprawki → plik w `art/` → wpis w `PROVENANCE.md`.
