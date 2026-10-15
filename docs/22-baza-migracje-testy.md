# F6.1 — baza, migracje i weryfikacja

Stan z 15.10.2026. Gotowy jest schemat trwałych danych wymagany przez F6.1. Następny krok F6.2 doda rejestrację, logowanie i obsługę sesji przez API.

## Dostarczony zakres

Cztery migracje w `migrations/` tworzą dziewięć tabel: `users`, `sessions`, `email_tokens`, `unlocked_templates`, `avatars`, `collection`, `decks`, `deck_presets`, `ratings`. Zastosowane migracje pozostają niezmienne; SQLx sprawdza ich sumy kontrolne i blokuje równoległe uruchomienie migratora.

- Konta mają unikalne loginy i e-maile bez rozróżniania wielkości liter (`citext`), ograniczenia pól i miejsce na hash Argon2id. Identyfikatory UUID dostarczy aplikacja. Sesje web/aplikacji oraz tokeny e-mail przechowują 32-bajtowe skróty, typ i czas ważności.
- Postacie mają sloty 0–9, poziomy 1–20, nieujemne PD i PR, ograniczenia reinkarnacji oraz unikalne indeksy dla zajętego slotu i aktywnej postaci. Miękkie usunięcie zwalnia slot. Zwykła postać wymaga odblokowanego szablonu; cień nie dodaje szablonu do odblokowań (R-CIE-01/05).
- Kolekcja ma nieujemne ilości. Talie i zapisane zestawy przyjmują najwyżej 20 dodatnich identyfikatorów w jednowymiarowej tablicy bez `NULL`. Usunięcie postaci nie usuwa kolekcji.
- Ranking startuje od 1000, nie spada poniżej 1 i wymaga zgodności sumy wygranych, remisów i porażek z liczbą gier.

Wywołanie `server migrate` stosuje migracje i kończy proces. `run.bat` wykonuje je przed startem serwera. Normalny serwer nie wykonuje DDL i łączy się wyłącznie przez `DATABASE_URL`; osobne dane roli DDL można podać jako `MIGRATION_DATABASE_URL`. `/readyz` wymaga zastosowania najnowszej migracji wbudowanej w binarkę, a `/healthz` pozostaje kontrolą procesu.

Skrypty `deploy/postgres/roles.sql` i `privileges.sql` rozdzielają właściciela schematu `game_migrator`, konto aplikacji `game_app` i odczyt `game_readonly`. Konto aplikacji może zmieniać dane gry i odczytywać historię migracji, ale nie może zmieniać schematu ani tej historii. Hasła ustawia się poza repozytorium. Lokalny Docker korzysta z konta deweloperskiego; instrukcja wdrożeniowa jest w [CONTRIBUTING.md](../CONTRIBUTING.md).

## Weryfikacja

Końcowy wynik lokalny: **19/19 etapów `check.bat` (133 s)**, bez pominiętych etapów; **14/14 scenariuszy Playwright**, Lighthouse mobile **100/100/100/100**. Przeszły także clippy, testy Rust/WASM, walidacja treści, balans, typy i budowanie klienta, metadane SQLx oraz `/healthz` i `/readyz` z bazą. Zdalny workflow CI został rozszerzony; nie był uruchamiany z tej sesji.

Testy `crates/server/tests/database.rs` działają na osobnych bazach SQLx w PostgreSQL 18.6. Wynik: **10/10**. Sprawdzają:

1. Migracje na pustej bazie, zmianę gotowości 503 → 200 i bezpieczne ponowne uruchomienie.
2. Aktualizację z zachowaniem konta, sesji i istniejącej postaci.
3. Unikalność loginu/e-maila i ograniczenia danych kont.
4. Typy, skróty i daty sesji oraz tokenów; usuwanie danych zależnych.
5. Sloty, aktywną postać, własność szablonu, cienie i zakresy rozwoju.
6. Dwie równoległe próby zajęcia tego samego slotu — dokładnie jedna się udaje.
7. Ilości kolekcji, kształt i zawartość tablic kart oraz zachowanie kolekcji po usunięciu postaci.
8. Domyślny i minimalny ranking oraz zgodność statystyk.
9. Odrzucenie zmienionej sumy kontrolnej zastosowanej migracji.
10. Migracje jako `game_migrator`, DML jako `game_app`, odczyt jako `game_readonly` oraz odmowę niedozwolonych operacji. Role testowe powstają w wycofywanej transakcji.

Wersjonowane metadane `.sqlx/` umożliwiają kompilację bez PostgreSQL. CI oraz pełny `check.bat` wykonują migracje, testy bazy i `cargo sqlx prepare --check --workspace -- --all-targets --locked` z `SQLX_OFFLINE=false`. Zwykłe `cargo test --workspace` pomija testy wymagające PostgreSQL. Polecenia ręczne i wymagane uprawnienia są w [CONTRIBUTING.md](../CONTRIBUTING.md).

Przy pełnej kontroli poprawiono również brakujące tytuły dostępności 72 grafik kart SVG, korzystając z polskich nazw w katalogu treści.

## Granice tego kroku

F6.1 nie udostępnia jeszcze API kont ani lobby. Walidacja identyfikatorów względem wkompilowanej treści, tworzenie początkowej postaci i zapewnienie co najmniej jednej aktywnej postaci należą do operacji aplikacji. Limit kopii kart, posiadanie kart i najwyżej 10 zapisanych zestawów dojdą z F8. Ekonomia, historia rankingu, pojedynki i społeczność otrzymają tabele w swoich fazach planu. Otwarte pomiary sprzętowe F5 pozostają opisane w [raporcie F5](21-trening-offline-testy.md).
