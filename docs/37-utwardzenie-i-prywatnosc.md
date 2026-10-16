# F13 — utwardzenie, prywatność i obciążenie

Stan lokalnej implementacji: 2026-10-16. F13 nie jest pełnym odbiorem produkcyjnym: pomiary VPS, 24-godzinna próba S6, fizyczne telefony, czytnik ekranu i akceptacja prawna pozostają otwarte.

## Wykonane zmiany

- Testy `loadtest` używają rzeczywistego gatewaya, Protobuf, aktorów, transakcji i własnej usuwanej bazy PostgreSQL. Nie logują się na konta graczy. Konfiguracje są w `loadtests/`; `LOADTEST_DATABASE_URL` dopuszcza wyłącznie lokalną bazę. Raporty zawierają zakres, konfigurację, jednostki, próbkę i status błędu.
- Obciążenie ujawniło blokowanie globalnego huba przez zapis czatu, ruchy i wymiany oraz zaprzestanie odbierania zdarzeń podczas oczekiwania na własny RPC. Operacje niezależnych aktywności są wykonywane poza pętlą huba; pojedynczy WebSocket nadal zachowuje kolejność odpowiedzi i jednego zapisującego. Oczekiwanie na DB nie blokuje opróżniania zdarzeń. Regresja z celowo zablokowaną transakcją sprawdza ten przypadek.
- Obecność w pokojach używa skróconego profilu i współdzielonego zakodowanego komunikatu; własny pełny profil pozostaje pełny. Ponowne wejście do tego samego pokoju nie rozsyła fałszywego `joined`. Są limity kolejki, pokoju i liczby połączeń; wolny odbiorca jest rozłączany, zamiast zwiększać pamięć bez ograniczeń.
- Eksport danych jest strumieniowany w spójnym widoku bazy, z limitami czasu, liczby eksportów i współbieżności. Błąd SQL po rozpoczęciu transmisji nie udaje poprawnie zakończonego pliku. Eksport nie ujawnia haseł, tokenów, RNG ani adresów innych graczy.
- Dane konta można obsłużyć świeżą sesją ograniczoną do praw prywatności, także przy blokadzie konta. Taka sesja nie daje dostępu do gry, WebSocketu ani rotacji zwykłego tokenu. Usunięcie wymaga hasła, dokładnego loginu i potwierdzenia; karencja wynosi 30 dni. Obsłużono utratę odpowiedzi na żądanie i anulowanie wniosku.
- Zadanie usuwania czyści dane kontaktowe, poświadczenia i treści społeczne oraz redaguje profil w historycznych projekcjach. Księga i przebieg walk zachowują pseudonimowe identyfikatory potrzebne do integralności. Dokumenty nie nazywają tego pełną anonimizacją.
- Android zapisuje eksport przez systemowy selektor SAF, bez tokenu w JavaScript lub URL i bez jawnego pliku tymczasowego aplikacji. Sesja jest związana z kontem; zapis i anulowanie przetestowano na emulatorze.
- Dodano wersjonowane dokumenty PL/EN, wymagane potwierdzenie wieku i regulaminu przy rejestracji oraz historię akceptacji. `PUBLIC_RELEASE=1` wymaga kompletu danych operatora i zatwierdzenia aktualnej wersji dokumentów. Lokalny podgląd zawiera wyraźny status projektu.
- Informacje licencyjne obejmują 96 składników klienta (NPM, Rust/WASM, biblioteki standardowe i Android) oraz 254 składniki serwera w obrazie, w tym statyczny musl. Są pobierane na żądanie w przeglądarce. Wydanie zawiera rzeczywiste teksty licencji i pochodzenie brakujących w paczkach tekstów; nowa wersja narzędzi wymaga odświeżenia.
- CI obejmuje `cargo-deny`, audyt pnpm, OSV zależności Maven, pasywny ZAP oraz skan obrazów Grype. Grype działa bez gniazda Dockera i bez wykonywalnego programu na hoście: aktualizacja widzi tylko swój cache, właściwy skan działa bez sieci na kopii jednego obrazu.
- Axe sprawdza WCAG 2.2 AA na ekranach konta, lobby, gry, prywatności i administracji, bez listy wykluczeń. Automatyczny wynik nie zastępuje odsłuchu czytnikiem ani oceny ergonomii dotyku.

## Wyniki obciążenia i ograniczenia

Generator i serwer działały w jednym procesie na Windows, PostgreSQL w lokalnym Dockerze. Inne uruchomione przez właściciela kontenery pozostały nietknięte. Te wyniki wykrywają regresje i potwierdzają integralność, ale nie certyfikują limitu CPU/RSS docelowego VPS.

| Próba | Wynik lokalny | Dowód w ignorowanym `target/` |
|---|---|---|
| 5000 WS, 2000 trwałych pojedynków | 6000 rund, p95 rundy 44,3 ms; wznowienie 2500 sesji 5,04 s; 2000 rozliczeń i zgodna księga | `f14-duels-2000-final.json` |
| 10 turniejów po 64 graczy | 320 równoczesnych walk pierwszej rundy; start 21,09 s z obowiązkowym oczekiwaniem 20 s | `f14-tournaments-10.json` |
| 200 równoległych zakupów/otwarć i 200 wymian | ponowienia nie duplikują operacji; 200 wpisów rozliczenia wymian; zgodna księga; p95 całego przepływu wymiany z ośmioma RPC 5,11 s | `f14-economy-streaming.json` |
| 5000 graczy we wszystkich pokojach, 84 wiadomości/s, 5% zmian obecności/min | 841 wiadomości/10 s; p95 dostarczenia 38,8 ms; brak utraty sesji; wznowienie połowy sesji **13,67 s** | `f14-lobby-baseline.json` |
| Ta sama próba, cztery wątki Tokio | wznowienie połowy sesji **12,82 s**, nadal powyżej 10 s | `f14-lobby-four-workers.json` |

Próba 500 wiadomości/s z pełnymi pokojami przekroczyła limit zaległości jednej sekundy (`f14-lobby-streaming.json`). Nagłe poddanie wszystkich 2000 walk naraz również nie spełniło czasów odpowiedzi; zwykła próba S2 kończy aktywności z jawną współbieżnością sprzątania 32. Nie wolno prezentować tych przeciążeń jako zaliczonych. Konfiguracji stresowych nie obniżono w celu uzyskania zielonego raportu.

`loadtests/soak.toml` uruchamia kolejne cykle walk przez 24 godziny, równolegle z czatem. Krótki test nie jest odbiorem S6; pełna próba wymaga wydzielonego hosta i zachowania metryk pamięci.

## Odbiór przed publicznym startem

Wymagane są: próby S1–S6 na wybranym VPS, domknięcie burzy wznowień w pełnych pokojach i przeciążenia czatu, pomiary LCP/INP/FPS na fizycznych telefonach, TalkBack/czytnik ekranowy, rzeczywista konfiguracja dostawców i okresów retencji, przegląd nazw/grafik oraz zatwierdzenie dokumentów przez właściciela i prawnika. ZAP lokalny jest podstawowym skanem pasywnym, a nie niezależnym testem penetracyjnym uwierzytelnionych ról.

Gość nadal działa wyłącznie w pamięci frontendu: nie tworzy konta ani rekordu telemetrii, nie wysyła danych gry do API i nie zapisuje postępu. Pobranie statycznego klienta/PWA nie jest zapisem danych konta gościa.

## Końcowa weryfikacja lokalna

117 testów PostgreSQL, 29 Vitest i 26 Playwright przeszło; Lighthouse mobile 100/100/100/100. W pełnym przebiegu `check.ps1` początkowo nie przeszły dwa kroki (brak `Debug` w nowym teście i formatowanie dwóch skryptów). Zostały poprawione; ponowione `cargo clippy --workspace --all-targets --locked -- -D warnings`, fmt i Biome są zielone. Nie ukryto błędów w surowym pierwszym logu. Typy wszystkich pakietów i build przeglądarki przeszły ponownie po zmianach pakowania Androida.

Budżety Brotli: startup JS 41,331/70 KiB, walka 37,573/40 KiB, konto/lobby 44,844/45 KiB, cały offline PWA 199,786/200 KiB. Dotychczasowych limitów nie podnoszono.

Audyt pnpm oraz cargo-deny nie zgłaszają podatności blokujących. Android OSV: 177 współrzędnych Maven, 0 zgłoszeń. Pasywny ZAP: 0 problemów wysokich/krytycznych; pięć typów niskich/informacyjnych pozostaje w raporcie. Końcowy Grype (baza 2026-10-15, bez wykluczeń): serwer **0 zgłoszeń**, PostgreSQL/pgBackRest **0 wysokich/krytycznych, 4 średnie**. Średnie dotyczą BusyBox (trzy wystąpienia jednego CVE) i biblioteki użytkownika narzędzia gosu. Nie są opisane jako usunięte. Pierwsze obrazy Debian wykazały rzeczywiste problemy; finalne obrazy i uzasadnienie minimalizacji są w ADR-020. Surowe raporty: `target/grype/accepted-0.json` i `accepted-1.json`.

Końcowa krótka próba S6 (`final-soak-cycles.json`) trwała około 78 sekund, utrzymywała 60 WS, 10 równoczesnych walk i czat, wykonała siedem cykli walk oraz uzgodniła księgę. Potwierdza powtarzanie cykli, nie spełnia 24-godzinnego odbioru pamięci.

Scenariusz nocnej kontroli również wykonano lokalnie: 500 WS, 100 równoczesnych trwałych pojedynków, 500 rund, p95 rundy 25,36 ms, wznowienie 250 połączeń 0,51 s i zgodna księga (`target/nightly-load.json`). Workflow `load.yml` przygotowuje ten sam test codziennie i na żądanie; nie deklarujemy uruchomienia go na koncie GitHub właściciela.

Poprawka glibc wymagająca nowszej bazy była potwierdzona w [trackerze Debiana](https://security-tracker.debian.org/tracker/CVE-2026-5450). Końcowy serwer nie zawiera dynamicznego glibc. Opcja wyłączenia SFTP pochodzi z [konfiguracji pgBackRest 2.59.3](https://github.com/pgbackrest/pgbackrest/blob/release/2.59.3/meson_options.txt); nie zastosowano wyciszenia skanera.

Grype nie zastępuje analizy statycznie dołączonych bibliotek. Zależności Rust sprawdza cargo-deny, a wersja kompilatora jest przypięta. Rust 1.99 dołącza musl 1.2.5 z poprawkami wymienionymi w [skrypcie budowania toolchainu](https://github.com/rust-lang/rust/blob/1.99.0/src/ci/docker/scripts/musl.sh); jego tekst COPYRIGHT jest zachowany w obrazie. Zero zgłoszeń obrazu nie jest obietnicą braku wszystkich przyszłych podatności.
