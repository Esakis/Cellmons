# F9.5 — treść bety i pierwszy audyt balansu

Stan: 2026-10-15. Dodano 60 kart Forge (12 + 48), 10 regularnych klas i 3 klasy wysypiska. Łącznie 132 karty, 19 szablonów i 6 zestawów startowych. Każda karta ma test scenariuszowy, tekst PL/EN i ilustrację SVG. Postacie mają portrety i krzywe 1–20; trzy kompletne krzywe pochodzą z referencyjnego CSV (Masakrator, Ripper, Lambert), pozostałe z generatora z parametrami zapisanymi w TOML.

Referencyjny Masakrator zna na poziomie 20 wyłącznie Walkę 3. Test krzywych uwzględnia ten przypadek obok częstszego 3+2/3+3. Nie dodano mu bez podstaw drugiej szkoły. Nowe klasy nie mają zestawów startowych: tworzą pustą talię, a trening lokalny buduje legalną talię demonstracyjną. Test natywny/WASM uruchamia każdą klasę bez zestawu startowego.

## Audyt balansu

`cargo run --release -p sim --example roster -- 200`: 102 600 pojedynków, wszystkie 171 par 19 klas po 200 walk na poziomach 1, 10 i 20. Strony zmieniają się co walkę; seed i legalne talie są deterministyczne. Talie pochodzą z całej legalnej puli danej postaci, z limitem kopii. Boty L1, bez wydanych PR, dróg, specjalizacji i darów. Remis liczy się za pół wygranej. CSV: `target/roster-balance.csv`.

| Klasa (klucz) | Poziom 1 | Poziom 10 | Poziom 20 |
|---|---:|---:|---:|
| dune_wanderer | 48.9% | 60.2% | 52.9% |
| dawn_warden | 65.8% | 65.9% | 69.9% |
| ember_drake | 47.7% | 55.7% | 47.7% |
| star_adept | 70.2% | 27.0% | 27.3% |
| ash_widow | 60.3% | 50.8% | 46.3% |
| cogwright | 45.4% | 61.5% | 61.1% |
| storm_surveyor | 62.7% | 46.3% | 30.6% |
| stoneguard | 39.3% | 55.3% | 60.9% |
| tide_oracle | 70.9% | 63.8% | 52.4% |
| venom_scout | 46.1% | 63.5% | 62.7% |
| iron_reaver | 40.2% | 49.1% | 55.2% |
| rift_duelist | 47.0% | 42.8% | 64.0% |
| root_sage | 53.8% | 57.2% | 46.8% |
| glass_alchemist | 66.3% | 33.7% | 36.7% |
| dusk_stalker | 45.3% | 67.7% | 69.8% |
| dawn_lancer | 40.2% | 50.1% | 58.6% |
| cinder_knight | 13.7% | 25.0% | 31.2% |
| copper_distiller | 44.3% | 30.4% | 38.0% |
| ruin_keeper | 41.8% | 43.9% | 37.7% |

Audyt ujawnia konfiguracje odstające: na poziomie 1 słaby Rycerz żużlu (13,7%), na poziomach 10–20 słaby Mag, na poziomie 20 silni Kapłan i Tropiciel zmierzchu (około 70%). Wyniki wymagają kalibracji F10.5 i prób z dobieranymi taliami oraz PR. Nie dowodzą wyrównanego balansu ani nie przewidują zachowania graczy. Macierz sześciu klas na poziomie 1 z ich zestawami startowymi nadal spełnia osobny próg F3.5 (każda poniżej 60%).

## Grafiki i weryfikacja

Galerie: `art/forge-review.html` (12 kart), `art/forge-beta-review.html` (48 kart), `art/beta-portraits-review.html` (10 klas). Trzy portrety wysypiska są w `art/avatars`. Zrzuty wszystkich nowych galerii obejrzano lokalnie. Grafiki mają wpisy w `art/PROVENANCE.md` i oczekują na odbiór właściciela. Nie oznaczono ich jako zatwierdzonych.

Offline przed dalszą optymalizacją: 198,5/200 KiB; ilustracje kart: 25,3/80 KiB. Nie zwiększono limitów. Rozszerzenie treści F10 będzie wymagało dalszej optymalizacji.

Pełny przebieg `target/beta-complete-check.log` zaliczył Rust/WASM, treść, macierz startową, AI, bazę, typy, budowanie, Vitest i Lighthouse. Przegląd usunął formatowanie galerii oraz błąd linku weryfikacyjnego przy już otwartej stronie konta. Osiem testów konta/lobby/sklepu/wymiany przeszło ponownie w obu projektach (`target/beta-review-e2e.log`), pozostałe 16 w pełnym przebiegu. Nowy test R-KNT-02 sprawdza wymóg e-maila także przed rozliczeniem wymiany (`target/beta-verified-trade-tests.log`). Łącznie 58 testów PostgreSQL.

F9.6 wymaga hosta, poczty/Turnstile, zaproszonych testerów i dwóch tygodni rzeczywistych prób; nie przeprowadzono jej lokalnie. Odbiór grafik, kalibracja i publiczne kryteria pozostają otwarte.

Przed F10 usunięto z ładunku prezentacyjnego klienta powtórzoną kopię efektów i krzywych rozwoju. Pełne reguły nadal są w WASM, a `content_version` nadal obejmuje całe definicje źródłowe. Test regresji potwierdza zmianę wersji nawet przy zmianie efektu niewidocznej w samych parametrach tekstu. WASM pozostał bajt w bajt identyczny. Usunięto też nieużywane funkcje ułamków fontu Inter; testy rastra nie wykazały zmiany zwykłego tekstu.

Po optymalizacji: offline 194,2/200 KiB, fonty 33,9/40 KiB, WASM 59,9/90 KiB. Testy kompilatora, typy, Vitest i clippy przeszły; 12 testów treningu/kolekcji/dotyku potwierdziło działanie lżejszych danych prezentacyjnych (`target/content-payload-e2e.log`).
