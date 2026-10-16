# Turnieje: zapisy, drabinka i rozliczenie

F11.1–F11.2, 2026-10-16. Turnieje graczy oraz turnieje klanowe działają w Zamku, na rzeczywistych aktorach pojedynków. Parametry obejmują liczebność, wpisowe, limit walk w fazie, poziomy i reinkarnacje, ograniczenie do własnego klanu oraz równe składy zaproszonych klanów. Tworzenie turniejów systemowych z nagrodami innymi niż pula wpisowego pozostaje w zakresie panelu administratora F12.1.

## Trwałość i pieniądze

Migracja 016 dodaje turnieje, uczestników, potwierdzenia żądań i harmonogram walk. Wpisowe trafia na rachunek `tournament:<uuid>`. Odroczone ograniczenia PostgreSQL sprawdzają zgodność salda rachunku z niezmienną księgą; okresowe uzgodnienie obejmuje teraz także te rachunki. Nie ma odrębnego licznika puli, który mógłby rozejść się z pieniędzmi.

Zapisy rezerwują aktywną postać i zamrażają profil walki oraz talię. Przed startem rezygnacja oddaje całe wpisowe. Zakończony turniej wypłaca całą pulę zwycięzcy; zwycięski klan otrzymuje równy podział z deterministycznym przydziałem reszty. Niewypełnione zapisy wygasają po 24 godzinach i zwracają wszystkie opłaty. Utracona odpowiedź nie pobiera ponownie wpisowego, nie powtarza zwrotu i nie zapisuje ponownie osoby, która już odeszła.

Blokady użytkowników są pobierane w kolejności UUID przed blokadą turnieju. Harmonogram ponownie sprawdza pełny skład po zablokowaniu agregatu, dzięki czemu równoczesny ostatni zapis nie pomija nowej osoby. Tworzenie dziennika walki w aktywności wymaga dokładnie pasującej blokady rodzaju i właściciela; nie omija ogólnej ochrony przed drugą walką.

## Drabinka i gotowość

Prywatne ziarno losuje pary, wolne losy i rozstrzygnięcia remisów. Nie jest wysyłane klientom. Seria przyznaje 2 punkty za wygraną i po 1 za remis; kończy się wcześniej wyłącznie przy przewadze niemożliwej do odrobienia. Klan nie awansuje, dopóki nie rozliczą się już zarezerwowane walki jego członków. Awansuje cały zwycięski skład, także członkowie, którzy przegrali własne pojedynki.

Pierwsza walka jest planowana po 20 s, następne po 15 s od powrotu obu osób. Gotowość potwierdza aktywne połączenie WebSocket w Zamku; wyjście z pokoju i rozłączenie ją kasują. Ponowienia pulsu zachowują czas pierwszego powrotu. Dzierżawa właściciela harmonogramu zapobiega równoczesnemu uruchomieniu na dwóch instancjach. Przejęcie po restarcie daje nowy czas na gotowość.

Uzupełnienia zasad Z-TUR-READY/ABS/PIN/LIM są zapisane w rejestrze. Jednostronna nieobecność tworzy prawdziwy walkower z normalnym rozliczeniem turniejowym. Obustronna nieobecność nie produkuje remisowych PD ani rankingu. Gdy żaden skład nie może wygrać, pula trafia do ujścia księgi. Rezygnacja po starcie wymusza pozostałe walkowery i nie oddaje opłaty.

## Przegląd i sprawdzenie

- Siedem testów PostgreSQL obejmuje prawdziwy turniej 32 botów, osiem klanów po cztery boty, wpisowe i wypłaty, serie po 100 konkurujących żądań, wygaśnięcie, gotowość, restart oraz obustronną nieobecność. Obie symulacje rozgrywają legalne ruchy L1 prawdziwym silnikiem, zapisują rundy i weryfikują dzienniki. Każdy wynik rozliczany jest ponownie w celu sprawdzenia idempotencji.
- Pięć scenariuszy drabinki sprawdza m.in. liczebności 2–64, serie remisów, wolne losy, czekanie na zarezerwowane walki i wykluczenie rezygnującego z wolnym losem z końcowego zwycięstwa.
- HTTP wymaga sesji i potwierdzonego e-maila do uczestnictwa, odrzuca obce pochodzenie i nieznane pola oraz nie eksportuje ziarna. Test WebSocket potwierdza gotowość wyłącznie w Zamku oraz jej usunięcie przy wyjściu i zerwaniu połączenia.
- Przeglądarka desktop/Pixel 5 przechodzi utworzenie z utraconą odpowiedzią i ponowieniem tego samego klucza, zapis, rezygnację, ponowny zapis, start obu klientów, rzeczywistą walkę, wynik oraz odczyt po przeładowaniu.

Przegląd znalazł i naprawił odtwarzanie całego panelu przy aktualizacji profilu lobby, użycie starego identyfikatora postaci po jej poświęceniu na cień oraz gubienie wybranego turnieju po zamknięciu wyniku walki. Poprawiono też przejście klawiaturą do treści: nie zmienia trasy routera, przenosi fokus i pozostaje wizualnie ukryte do użycia klawiatury.

Pełny przebieg kontroli obejmował 80 testów PostgreSQL, 25 Vitest, 24 testy przeglądarkowe, natywne/WASM, SQLx, budżety, AI i Lighthouse 100/100/100/100. Wszystkie kontrole funkcjonalne przeszły; jedyny problem lint wymagał zastosowania przycisku do przenoszenia fokusu. Po poprawce ponowiono lint i test klawiatury. Nie przedstawia się pierwszego przebiegu z błędem lint jako całkowicie zielonego.

Moduł turniejów ma około 6,4/20 KiB Brotli i jest ładowany dopiero w Zamku. Zachowano limity 45 KiB konta/lobby i 200 KiB aplikacji offline; moduł turniejów nie jest zapisywany w pakiecie offline. Gość nie uzyskuje dostępu do turniejów ani ich API. Pełny budżet trasy mapy i obciążenie wielu równoczesnych turniejów wymagają pomiarów F13.
