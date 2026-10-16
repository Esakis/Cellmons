# 29. Reinkarnacje, cienie i katalog postaci

F10.1, F10.2 i F10.4 są przygotowane. F10.3, F10.5, odbiór grafik i dalsze fazy pozostają w pracy.

Reinkarnacja wymaga poziomu 20, 65 000 PD i aktualnej rewizji. Gracz wybiera kolejno drogę, specjalizację i dar. Każdy reset zeruje PD, poziom i wydane PR; przyznaje 2/4/6 nowych PR. Zapisana talia, kolekcja i wcześniejsze wybory pozostają. Dar działa od początku walki bez zużywania lub przyznawania egzemplarza w kolekcji. Online i lokalny WASM używają wybranego profilu, specjalizacji i daru.

Poświęcenie Ascent tworzy cień w tym samym slocie, także przy pełnej puli. Źródłowa postać znika; cień przejmuje aktywność, zaczyna na poziomie 1 i ma pustą talię. Operacja nie odblokowuje szablonu do darmowego powielania. Każda reinkarnacja cienia wymaga innej własnej postaci o dokładnie jeden stopień wyższej, bez bieżącej biegłości w szkole cienia. Zużycie aktywnej ofiary atomowo aktywuje cień. Operacje respektują blokady aktywności i pojedynku.

Migracja 014 dodaje historię reinkarnacji i poświęceń. Reset, usunięcie ofiary, aktywność i audyt są w jednej transakcji pod blokadą użytkownika. Serwer sprawdza zgodność rodzica drogi, stopnia, specjalizacji, daru i flag cienia. Stara rewizja lub błędna ofiara nie powoduje częściowego zapisu.

Katalog ma 31 szablonów i 62 drogi z tekstami PL/EN. Cienie dostają +4 Ż/+4 W na poziom, 21 PR i nie mają automatycznego wzrostu ataków/obron. Perfekcja osiąga jedną szkołę na poziomie 4; Obłęd profil 3/2/2. Testy kontrolują sumy statystyk i zasobów dróg, harmonogramy PR, kompletność par i aktywne dary. Brakujące archiwalne dane uzupełniono jawnie jako Z-PATH-1, Z-CIE-P, Z-GFT-2/3; wymagają kalibracji.

Interfejs PL/EN pokazuje warunki, drogi z parametrami, specjalizacje, teksty darów, odpowiednie ofiary i potwierdzenie trwałych skutków. Biblioteka pokazuje 24 karty i kolejne porcje; wyszukiwanie obejmuje cały katalog. Gość używa wyłącznie RAM, bez API i zapisu; trening nie daje PD, więc gość nie reinkarnuje.

Sprawdzenia implementacji:

- Pięć testów PostgreSQL rozwoju: resety, trwałe wybory, rewizje, blokady, pełna pula, wycofanie po konflikcie, cudze ofiary i błędny profil. Po 100 jednoczesnych żądań na każdy etap cienia nie zużywa ofiary dwa razy.
- Osobny test walki kontroluje wycenę cienia jak Ascent po obu stronach wzoru PD i przyznanie 21. PR. Test wszystkich dróg sprawdza darmowe dary; WASM odrzuca dar spoza drogi.
- Każda ze 165 kart ma scenariusze liczbowe. Nowe 33 umagicznienia Relic mają własne teksty i ilustracje.
- Desktop i Pixel 5: trzy reinkarnacje, trening z darem i tworzenie cienia na istniejącym koncie. Gość buduje talię offline, przeszukuje późne karty i nie wysyła API.
- Mały kodek treningu używa oficjalnych `BinaryReader`/`BinaryWriter`; pola generuje z deskryptora `training.proto`. 600 porównań ze standardowym kodekiem obejmuje wszystkie żądania i odpowiedzi, wartości graniczne, Unicode, puste wiadomości, powtórzenia oraz błędne dane.
- Scalenie Cinzel zachowuje te same piksele w 40 porównaniach PL/EN. Wejścia i OFL zachowano; zwykły build nie wymaga Pythona.

Pełne `scripts/check.ps1` przeszło: formatowanie, clippy, treść, testy Rust, macierz startowa, hierarchia AI, zgodność native/WASM, typy, 19 testów Vitest, build, Protobuf, 64 testy PostgreSQL, 24 scenariusze Playwright, Lighthouse 99/100/100/100 i health/ready serwera. Log: `target/f10-complete-check.log` (245 s).

Pomiar paczek: offline 192,5/200 KiB, start JS 50,5/70, walka JS 14,6/40, WASM 64,0/90, ilustracje kart 28,1/80, moduły online 43,1/45. Logi robocze: `target/f10-*`.

Grafiki: 33 emblematy Relic, 12 nowych portretów bazowych i 62 warianty dróg. Galerie: `art/relic-review.html` i `art/full-portraits-review.html`. Wszystkie 107 obrazów zdekodowano i przejrzano. Klient składa tę samą nakładkę z portretem bazowym. Odbiór właściciela pozostaje otwarty.

Do zakończenia F10 pozostają: druga połowa kart w pięciu edycjach, raporty balansu i tempa pełnego zestawu, korekty odchyleń oraz odbiór grafik. Beta publiczna, fizyczne telefony, konta sklepów i docelowe wdrożenie są zewnętrznymi kryteriami planu.

Kontynuacja: [pierwsza paczka Covenant i model tempa](30-covenant-i-tempo-rozwoju.md), 198 kart oraz porównanie wariantów PD.
