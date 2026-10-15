# F6.3–F7.1: lobby, WebSocket, wyzwania i przegląd kodu

Data: 2026-10-15. Zakres: cztery kolejne punkty po F6.2. Przegląd obejmował nowy kod oraz jego granice z istniejącym uwierzytelnianiem, silnikiem, workerem, PWA, bazą i konfiguracją CI. To przegląd wykonany podczas implementacji, bez zewnętrznego audytu.

## Wykonane punkty

| Punkt | Wynik |
|---|---|
| F6.3 | `/ws`: cookie web i jednorazowy bilet aplikacji, dokładny Origin, Hello/Welcome, wersje protokołu i treści, heartbeat, resume, przejęcie sesji, ograniczone kolejki, limity IP i ramki |
| F6.4 | Aktory pomieszczeń, obecność, publiczny/prywatny/klanowy czat, filtry, AFK, znajomi, ignorowani, poczta z paginacją i odczytem/usuwaniem przez właściciela |
| F6.5 | Lobby PL/EN, mapa, sortowanie graczy, profil, komendy, poczta, kontakty, reconnect; rzeczywista rejestracja dwóch klientów w Playwright |
| F7.1 | Wysyłanie, przyjęcie, odrzucenie, anulowanie i wygaśnięcie wyzwania; filtry, kontrola właściciela, blokady, atomowa rezerwacja przygotowanego pojedynku |

Akceptacja zapisuje w RAM niezmienny zestaw dwóch postaci i talii sprawdzony przez silnik oraz tajne ziarno. `claim_duel` odbiera rezerwację dokładnie raz, a `finish_duel` zwalnia blokady. Nieodebrana rezerwacja wygasa po 30 s. **Aktor rozgrywający walkę, ruchy online, odtwarzanie po restarcie i ekran walki online należą do następnych punktów F7.2/F7.5.** Interfejs informuje o tym po akceptacji. Lista ostatnich pojedynków pozostaje pusta do wdrożenia historii walk. Endpointy zmiany postaci/talii F8 muszą respektować blokadę wysłanego wyzwania i pojedynku; obecnie takich endpointów nie ma.

Gość nie tworzy sesji, nie łączy się z WebSocket, nie korzysta z API i nie zapisuje danych ani postępu. Zasoby treningu pozostają w publicznym cache PWA; dane konta, wiadomości i token resume nie trafiają do pamięci trwałej przeglądarki.

## Znaleziska przeglądu i poprawki

| Problem | Poprawka i sprawdzenie |
|---|---|
| Wyzwanie pozostawało po opuszczeniu Areny | Zmiana/opuszczenie pokoju anuluje oferty; przyjęcie ponownie sprawdza Arenę. Test sieciowy próbuje przyjąć anulowaną ofertę |
| Resume zerował AFK | Zachowanie flagi między połączeniami; regresja z publicznym profilem po resume |
| Przepełnienie kolejki mogło zgubić Leave/Update | Operacje obecności oczekują na ograniczoną kolejkę; wolny odbiorca jest odłączany przez limit wyjścia |
| Rozłączenie sieci usuwało klienta i token resume | Otwarty ekran pozostaje zamontowany; nasłuch online/offline w kliencie. E2E wraca do Areny bez ponownego wyboru |
| Opóźniony `/api/me` mógł nadpisać nowe konto | Nowa generacja i anulowanie poprzednich żądań przy logowaniu/wylogowaniu; regresja Vitest |
| Ponowne logowanie usuwało stary cookie w SQL, ale nie zamykało od razu jego WebSocket | Unieważnienie po zatwierdzeniu transakcji; analogicznie logout, reset, rotacja i usunięcie sesji. Heartbeat dodatkowo sprawdza bazę |
| CSP aplikacji Android nie dopuszczało `wss:` API | Dodanie dokładnego odpowiednika WSS dla skonfigurowanego HTTPS origin; ścieżka biletu sprawdzona testem rzeczywistego upgrade |
| Filtr „klanowe i prywatne” odrzucał także wiadomości pokoju klanowego | Uwzględnienie prywatnego pokoju klanu w dostarczaniu i historii; regresja wraz z odebraniem członkostwa |
| Progi tytułów odbiegały od R-RNK-06, a UI pokazywało klucze | Poprawienie progów 400/600/800/1500 i lokalizacja tytułów PL/EN |
| Konwersje identyfikatorów treści mogły obcinać liczby | `try_from` dla postaci, poziomu, daru i kart; odrzucenie `CONTENT_MISMATCH` zamiast wyboru innej definicji |
| Pakiet offline przekroczył 200 KiB | Konto i lobby ładowane na żądanie; graf Vite zachowuje wspólne zależności w budżecie offline. Po równoległym rozszerzeniu CSS skrócono też wynikowe selektory CSS Modules (nazwy w źródłach bez zmian). Limit pozostał 200 KiB |
| Nowe formaty Protobuf powiększały każdy wariant kolejki | Duże warianty i komunikat Attach są opakowane w Box; clippy bez wyciszania ostrzeżeń |
| Heartbeat pomijał błąd wewnętrzny wysłania ping | Sprawdzenie obu poziomów wyniku wysyłki |
| Puste pokoje klanowe pozostawały bez końca | Usuwanie aktorów bez aktywnych członków; publiczne pokoje mają stały, ograniczony zbiór |
| Awaria bazy mogła zatrzymać aktora lub pozostawić przeterminowane żądania w kolejce | Limit oczekiwania i wykonania attach/polecenia 5 s; pomijanie żądań z porzuconym odbiorcą; rollback transakcji po timeout |
| Anulowany odbiór rezerwacji przed przekazaniem odpowiedzi mógł zostawić blokadę | Pominięcie porzuconego odbiorcy i cofnięcie claim, gdy dostarczenie nie doszło do skutku. Test jednostkowy sprawdza też, że duplikat nie odblokowuje poprawnie odebranego pojedynku |
| Dodanie istniejącego kontaktu przy pełnej liście zwracało błąd | Limit liczy pozostałe wpisy, zachowując idempotencję |
| Nadmierna równoległość E2E powodowała timeout, a retry zużywał limit rejestracji | CI uruchamia dwie przeglądarki równolegle; rozbudowany scenariusz dwóch kont ma limit 60 s i ślad także przy pierwszym niepowodzeniu. Rzeczywista rejestracja nie jest ponawiana na tej samej bazie |
| Wzorzec loginu formularza był nieprawidłowy przy HTML RegExp `v` | Poprawne escapowanie myślnika; serwer nadal niezależnie waliduje login |

Przejrzano także rozdzielenie tajnych danych walki od publicznego profilu, właściciela poczty, SQL parametryzowany, kolejność blokad użytkowników/awatarów/talii, CORS/CSRF, hashe sesji, cache Service Workera i granice worker/WASM. Nowa migracja jest dopisana; wcześniejsze pliki pozostały bez zmian.

## Weryfikacja

Test obciążeniowy utrzymuje 1000 uwierzytelnionych WebSocket równocześnie i sprawdza binarny, skorelowany Pong dla każdego. Lokalnie około 4,3 s w debug; limit IP jest podniesiony wyłącznie w konfiguracji testu. Nie jest to pomiar 1000 jednocześnie aktywnych rozmów ani wydajności produkcyjnego VPS.

Testy PostgreSQL tworzą własne bazy SQLx. Playwright używa osobnej bazy `game_e2e_<uuid>`, prawdziwego serwera i poczty przechwytywanej w RAM; teardown usuwa wyłącznie tę bazę i czeka na sprzątanie. Dane bazy deweloperskiej nie są czyszczone. Scenariusz dwóch klientów obejmuje tekst przypominający XSS, kontakty, pocztę, odrzucenie wyzwania, utratę/odzyskanie sieci i przeładowanie strony. Zrzuty do oceny układu: `target/lobby-chromium.png` i `target/lobby-mobile-chromium.png`.

Pełny zakres `check.bat` wykonano z PostgreSQL i Chromium. Równolegle drugi agent zmieniał ekran treningu; zachowano te zmiany, poprawiając w jego TSX jedynie formatowanie Biome. Ostatni zbiorczy przebieg zaliczył Rust, balans/AI, WASM, treść, typy, Vitest, protokół, migracje/SQLx i health checks. Zatrzymał budowanie na przekroczeniu budżetu przez nowy CSS (200,8 KiB); po minifikacji selektorów ponowiono zmienione kontrole web. Log zbiorczy: `target/check-four-complete.log`, poprawiony build: `target/four-compact-css.log`; ostatni build po kolejnej edycji CSS drugiego agenta: `target/four-final-build.log` (budżet nadal zaliczony).

Potwierdzone kontrole: 36/36 integracyjnych testów PostgreSQL (14 kont, 10 integralności bazy, 12 online), 14/14 Vitest, wszystkie reguły Rust i zgodność native/WASM; lint/format Rust, Biome i buf, aktualne metadane SQLx oraz kompilacja klienta. Budżet offline po równoległych zmianach: **199,9 / 200 KiB**, konto/lobby na żądanie: 14,5 / 45 KiB. Osobno po ostatniej poprawce przeszło 10 testów biblioteki serwera, w tym anulowany handoff; log `target/four-review-last.log`.

Po ostatniej poprawce: **22/22 E2E** (desktop i Pixel 5, 24 s), **Lighthouse 100/100/100/100** (performance, accessibility, best practices, SEO). Logi: `target/four-final-e2e.log`, `target/four-final-lighthouse.log`. Łącznie zaliczono wszystkie kontrole składające się na `check.bat`; poprawiony build i zależne od niego E2E/Lighthouse uruchomiono ponownie osobno, bez ponawiania niezmienionych, zaliczonych testów silnika.

## Ograniczenia i następny punkt

Następny punkt: **F7.2 — aktor pojedynku online**. Lobby działa na jednej instancji; oferty, obecność i resume są w RAM. SMTP/Turnstile wymagają konfiguracji produkcyjnej. Testy mobilne lobby wykonano w Chromium z emulacją Pixel 5, a nie na fizycznym telefonie ani w powłoce Androida. Sprzętowe pomiary F5 i trwały magazyn Keystore z F12A pozostają otwarte.
