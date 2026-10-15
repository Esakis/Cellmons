# F9.1–F9.4 — ekonomia, sklep, wysypisko i wymiana

Stan: 2026-10-15. Zaimplementowano cztery punkty domenowe i klienta PL/EN. F9.5 jest w toku, a zamknięta beta F9.6 nie została przeprowadzona. Gość nie ma portfela serwerowego, nie wykonuje żądań sklepu ani wymiany; jego kolekcja i trening pozostają w RAM.

## Portfel i księga

Konto otrzymuje 40 Dukatów w transakcji rejestracji. Wygrana arenowa daje 2 Dukaty, do 10 dziennie z tego źródła; pierwsza wygrana dnia dodaje osobne 10. Dzień jest wyznaczany według UTC w PostgreSQL. Rozliczenie należy do tej samej transakcji co wynik walki, a historia i powtórzenie wyniku korzystają z zapisanej nagrody. Trening nie daje waluty.

Każdy przepływ ma dwie przeciwne pozycje księgi, także przy wypłatach i spalaniu waluty. Odroczone ograniczenia bazy sprawdzają bilans i zgodność portfela z księgą przy zatwierdzeniu. Historia jest dopisywana; UPDATE i DELETE są odrzucane. Serwer co godzinę wykonuje uzgodnienie. Ręczny odczyt: `cargo run -p server -- reconcile` z `DATABASE_URL`; niezgodność daje niezerowy kod wyjścia i zagregowany raport, bez automatycznej korekty danych.

## Sklep i wysypisko

- Podstawowy: 12 kart (1 złota, 3 srebrne, 8 brązowych), 40 Dukatów. Pozostałe zestawy: 8 brązowych za 30, 4 srebrne za 40, 2 złote za 60. Parametry są w `content/boosters.toml`.
- Oferta zawiera cenę, identyfikator wersji i jawne pule z równymi szansami w obrębie rzadkości. Losowania są niezależne, z powtórzeniami. Kupiony zestaw zachowuje pulę z chwili zakupu. Seed pochodzi z systemowego CSPRNG i nie jest udostępniany klientowi.
- Żądania kupna, otwarcia i wyrzucenia mają klucz UUID i skrót treści. Ten sam klucz oddaje ten sam wynik; zmieniona treść z tym kluczem jest konfliktem. Wynik i operacja zapisują się atomowo. Otwieranie już otwartego zestawu również nie dodaje kart ponownie.
- Klient trzyma niepewne żądanie w RAM także po zmianie trasy. Ponowienie używa pierwotnego klucza. Zmiana konta usuwa ten stan. Baza pozostaje źródłem historii po przeładowaniu aplikacji.
- Wyrzucenie wymaga potwierdzenia liczby kopii. Punkty 5/25/100; każde 1000 odblokowuje kolejny z trzech szablonów specjalnych. Po wszystkich trzech kolejne progi dają po 40 Dukatów. Talie wszystkich postaci i zestawy zostają przycięte w tej samej transakcji. Czas rozliczenia dużej liczby progów zależy od liczby brakujących szablonów, nie od liczby progów.

## Wymiana

Zaproszenie wysyła się w Bazarze do obecnego gracza; ignorowanie, AFK i zajętość blokują zaproszenia. Mogą wymieniać się konta zarejestrowane z potwierdzonym e-mailem (R-WYM-05 i R-KNT-02). Warunek jest ponownie sprawdzany w transakcji przed zmianą oferty i rozliczeniem. Zaproszenie wygasa po 30 sekundach. Po przyjęciu obaj układają oferty kart i Dukatów; szablony i nieotwarte zestawy nie należą do protokołu oferty.

Każda zmiana oferty zwiększa jej wersję i wycofuje obie akceptacje. Potwierdzenie musi wskazać aktualną wersję. Powtórne potwierdzenie tej samej strony jest idempotentne. Po obu akceptacjach serwer ponownie sprawdza stan własności i środków, przenosi różnicę kart i waluty, przycina talie, zapisuje wynik i zwalnia blokady w jednej transakcji. Ograniczenie kolekcji lub brak środków wycofuje wszystkie te zmiany.

Każda wymiana ma aktora z kolejką 32 poleceń; rejestr mieści najwyżej 1024 aktorów. Właściciel ma odnawianą dzierżawę w bazie. Po jej utracie stary aktor nie może zatwierdzić operacji. Odzyskiwanie anuluje wyłącznie wymiany z wygasłą dzierżawą, nie przejmuje ofert do późniejszego rozliczenia. Wyjście z pokoju i rozłączenie anulują wymianę. Bezczynność przy ofertach: 120 s; całkowity czas: najwyżej 15 min. Zaproszenie blokuje zasoby zapraszającego; zasoby odbiorcy są blokowane po przyjęciu.

Wspólne `activity_locks` blokują jednoczesną walkę, zmianę postaci, talii, zakup, otwarcie i wyrzucanie. Wszystkie operacje najpierw blokują wiersze uczestników w porządku UUID. Zapobiega to wyścigom i odwróceniu kolejności blokad.

## Sprawdzone przypadki i przegląd

Testy PostgreSQL obejmują: 100 prób wydania tego samego salda, 100 ponowień zakupu i otwarcia, 100 ponowień wyrzucenia, progi 2995/3000/4000 punktów, unikalność trzech szablonów, przycinanie talii i zestawów, odrzucenie częściowego wyrzucenia, blokady aktywności, ukrycie zawartości nieotwartych zestawów, własność, starą cenę, niezbilansowaną księgę i próbę zmiany jej historii. Test rozkładu sprawdza 10 000 zestawów, skład rzadkości, deterministyczność i równomierność puli złotej.

Wymiana ma testy zmiany ofert po akceptacji, nieaktualnej wersji, 100 konkurencyjnych potwierdzeń, prób zużycia kart w trakcie, pełnego wycofania przy przepełnieniu kolekcji, odzyskiwania dzierżawy oraz dwóch prawdziwych klientów WebSocket. Test przeglądarkowy celowo gubi odpowiedź na zatwierdzony zakup, ponawia pierwotny klucz, otwiera dokładnie jeden zestaw i kończy wymianę dwóch kont. Przeszedł na desktopie i emulowanym Pixelu 5. Test gościa potwierdza brak żądań API/WS i zapisu kolekcji.

Przegląd poprawił m.in. utratę akceptacji przy zmianie oferty, obsługę niepewnego wyniku żądania, atomowe wycofanie częściowej operacji, obsługę wygasłych aktorów, czyszczenie blokad nieaktywnych kont, odrzucanie kart o ID poza zakresem `smallint` oraz trening nowego szablonu bez zestawu startowego. Ikony PWA zoptymalizowano bezstratnie (16619 → 7629 bajtów); porównanie zdekodowanych pikseli RGBA potwierdziło identyczność.

Kontrole pełnego repozytorium przeszły: Rust fmt/clippy i testy, walidacja treści, macierz startowa, cztery serie AI, natywne/WASM, typy, 15 testów Vitest, 57 testów PostgreSQL, 24 testy E2E, protokół, budowanie i serwer health/ready. Lighthouse mobile: 99/100/100/100. Paczka offline: 190,4/200 KiB, online: 27,5/45 KiB. Pierwszy pełny przebieg wykazał formatowanie jednego JSX oraz dwa stare testy liczby postaci; po poprawkach Biome, Vitest i oba projekty testu smoke przeszły ponownie (`target/f9-vitest-final.log`, `target/f9-smoke-final.log`). Pozostałe 22 E2E przeszły w pełnym przebiegu. Logi lokalne: `target/f9-complete-check.log`, `target/f9-junkyard-tests.log`, `target/f9-trade-tests.log`, `target/f9-e2e-retry.log`. Obrazy: `target/store-mobile-chromium.png`, `target/trade-mobile-chromium.png`, `target/junkyard-review.png`.

## Granice odbioru

Obecnie 84 karty i 9 szablonów: 72 karty MVP, pierwsze 12 kart Forge oraz 3 postacie wysypiska. Każda karta ma test scenariuszowy. Pozostała treść bety, raport jej balansu i odbiór nowych ilustracji są częścią F9.5. Rejestr pochodzenia wyraźnie oznacza nowe grafiki jako oczekujące na właściciela.

Nie wykonano publicznej bety z 20–50 testerami przez dwa tygodnie, zgłoszenia znaku, zakładania kont organizacji ani publikacji Google Play. Nie ma skonfigurowanego docelowego hosta i domeny. Te działania wymagają rzeczywistego środowiska, kont oraz czasu; lokalne testy ich nie zastępują.
