# Praca z repozytorium

Jak przygotować komputer, uruchomić grę lokalnie, sprawdzić zmiany i zgłosić je do przeglądu. Architekturę i plan opisuje `docs/` (zacznij od `README.md`).

## 1. Wymagane narzędzia

| Narzędzie | Wersja | Skąd |
|---|---|---|
| Rust | z `rust-toolchain.toml` (1.99.0, z rustfmt, clippy i targetem `wasm32-unknown-unknown`); rustup instaluje ją sam przy pierwszym poleceniu `cargo` | https://rustup.rs |
| Node.js | 24 LTS (`.nvmrc`) | https://nodejs.org |
| pnpm | z pola `packageManager` w `package.json` (12.11.2) | polecenie `corepack enable pnpm` |
| Docker Desktop | aktualna | https://www.docker.com/products/docker-desktop |
| wasm-bindgen-cli | taka jak crate `wasm-bindgen` w `Cargo.lock` (0.2.129) | `cargo install wasm-bindgen-cli --version 0.2.129 --locked` |
| sqlx-cli | 0.9.0, do pełnych kontroli bazy | `cargo install sqlx-cli --version 0.9.0 --locked --no-default-features --features rustls,postgres` |

Na Windowsie Rust potrzebuje narzędzi C++ z Visual Studio (Build Tools, składnik „Desktop development with C++”).

`wasm-bindgen-cli` dostarcza `wasm-bindgen-test-runner`, który uruchamia testy silnika w WebAssembly (Node). Jest wymagany przez `run.bat` i `check.bat` także do przygotowania klienta. Po zmianie wersji crate'a `wasm-bindgen` trzeba zainstalować tę samą wersję narzędzia.

## 2. Uruchomienie i sprawdzanie zmian

| Polecenie | Działanie |
|---|---|
| `run.bat` | uruchamia bazę (PostgreSQL w Dockerze), stosuje migracje, uruchamia serwer gry i klienta, potem otwiera przeglądarkę na http://localhost:5173 |
| `run.bat -NoBrowser` | to samo bez otwierania przeglądarki |
| `run.bat -Stop` | zatrzymuje bazę; dane zostają w woluminie Dockera |
| `check.bat` | wszystkie kontrole z CI: Rust, silnik do WebAssembly z testem natywne = WASM, klient, proto, testy e2e i serwer z bazą |
| `check.bat -Quick` | szybsze sprawdzenie, bez testów e2e, Lighthouse, migracji i testów bazy oraz testu serwera z bazą |
| `cargo run -p sim -- duel [ziarno] [--a L0\|L1\|L2\|L3] [--b L0\|L1\|L2\|L3]` | pojedynek botów (domyślnie L1 przeciw L1) na losowych postaciach i kartach testowych, opisany runda po rundzie; `--starters nazwa,nazwa` wybiera prawdziwą treść |
| `cargo run --release -p sim -- run [--n 1000] [--seed 0] [--a L1] [--b L1]` | seria pojedynków botów: wygrane, remisy, powody końca, średnia liczba rund, przewaga inicjatywy i czas |
| `cargo run -p content-compiler -- check` | sprawdza treść w `content/` i to, czy wygenerowany kod w `engine-content` jest aktualny |
| `cargo run -p content-compiler -- build` | po zmianie treści: generuje `crates/engine-content/src/generated.rs` i pliki klienta w `target/content/` |
| `cargo run -p content-compiler -- curves --weights 35,30,20,15 --school 5:combat=2` | wypisuje tabelę rozwoju postaci (poziomy 2–20) do wklejenia w `content/avatars/*.toml` |
| `cargo run --release -p sim -- matrix [--n 2000] [--level 1] [--bot L1]` | macierz wygranych postaci startowych z taliami startowymi; kod wyjścia 1, gdy któraś ma ponad 60% |
| `cargo run --release -p sim -- ai [--n 1200] [--level 1] [--seed 0]` | F4: L2–L1 i L3–L2 na równych postaciach, osobno boty domyślne z taliami startowymi i profile z ich własnymi taliami; rotacja stron, 95% CI i czasy decyzji; kod 1 poniżej progów 60% / 55% lub przy p95 L3 > 50 ms |
| `cargo run -p sim -- duel [ziarno] --starters dune_wanderer,ember_drake` | pojedynek postaci startowych opisany runda po rundzie |
| `cargo bench -p engine` | pomiar rozstrzygania rundy i całego pojedynku (`docs/06-silnik-gry.md`, rozdz. 10) |

- Serwer i klient działają w osobnych oknach; zamknięcie okna zatrzymuje usługę.
- W wersji deweloperskiej panel „Środowisko lokalne” w rogu strony pokazuje, czy działają serwer gry i baza.
- Konfiguracja lokalna jest w pliku `.env`, który `run.bat` tworzy z `.env.example`. Plik `.env` nie trafia do repozytorium.
- Lokalna baza używa portu 5433, a serwer gry portu 8090, bo porty domyślne (5432 i 8080) często zajmują inne programy.
- `check.bat` uruchamiaj po każdym kroku planu i przed każdym commitem. Kończy się podsumowaniem i kodem wyjścia 0 albo 1.
- Kontrola AI buduje `sim` w trybie release i rozgrywa cztery serie po 1200 walk: dwie pary poziomów dla botów domyślnych i dwie dla rzeczywistych profili. Profile z `content/bots.toml` są wbudowane w `engine-ai`; ich składnię, parametry, rotację postaci i legalność talii sprawdza `cargo test`.
- Poza Windowsem kontrole można uruchomić ręcznie; pełną listę poleceń zawiera `.github/workflows/ci.yml`.

### Klient i trening offline (F5)

`pnpm --filter web dev` oraz `build` automatycznie przygotowują treść, SVG i zoptymalizowane wiązania WASM. `wasm-bindgen-cli` 0.2.129 jest teraz wymagany także do budowania klienta. Na czystym checkout przed samym `typecheck` albo `test` wykonaj `pnpm --filter web prepare:assets`. Wygenerowane pliki treści/WASM nie trafiają do Git; typy TS Protobuf są wersjonowane i aktualizowane przez `pnpm proto:generate`.

`pnpm --filter web build` sprawdza budżety rozmiaru, `pnpm audit:web` sprawdza Lighthouse mobile (próg 95 w czterech kategoriach, wymaga zainstalowanego Chromium Playwright). Pełny `check.bat` wykonuje oba kroki i scenariusze Playwright na komputerze oraz w emulacji telefonu. `-Quick` pomija także Lighthouse. PWA/offline testuj przez build i `pnpm --filter web preview`; serwer deweloperski nie rejestruje Service Workera.

Android debug: `pnpm --filter mobile sync`, następnie `mobile/android/gradlew.bat -p mobile/android assembleDebug`. Potrzebne JDK 21 (`JAVA_HOME`) i Android SDK (`ANDROID_HOME`, API 36). Wariant Android wyłącza Service Workera, bo zasoby są w APK. Po nim ponów zwykły build web do sprawdzenia PWA. Procedurę i otwarte pomiary sprzętowe opisuje [raport F5](docs/21-trening-offline-testy.md).

### Baza i migracje (F6.1)

`run.bat` stosuje migracje przed startem serwera. Ręcznie: `cargo run -p server --locked -- migrate` (wczytuje `.env`). Zwykły start serwera nie wykonuje DDL; `/readyz` zwróci 503 do czasu zastosowania wymaganej migracji. Pliki `migrations/` są dopisywane; zastosowanych plików nie edytujemy.

Pełny `check.bat` uruchamia migracje, testy PostgreSQL (10 bazy, 14 kont i testy gateway/lobby) i kontrolę metadanych SQLx; `-Quick` pomija ten krok. Testy korzystają z osobnych, tymczasowych baz SQLx. Nie czyszczą danych bazy deweloperskiej. Domyślne `cargo test --workspace` pomija te testy, więc działa bez PostgreSQL. Uruchomienie obu grup w PowerShell:

```powershell
docker compose -f deploy/docker-compose.dev.yml up -d --wait
$env:DATABASE_URL = 'postgres://game:game@127.0.0.1:5433/game'
cargo test -p server --tests --locked -- --ignored
```

Konto testowe potrzebuje `CREATEDB`, a test uprawnień — konta administratora PostgreSQL, jak w lokalnym Dockerze i CI. Ten test tworzy role w transakcji, którą wycofuje. Używaj dedykowanego klastra deweloperskiego lub CI.

Kompilacja domyślnie korzysta z wersjonowanych metadanych `.sqlx/`, bez połączenia z bazą. Po zmianie zapytania makra SQLx lub schematu najpierw zastosuj migracje, następnie odśwież metadane i dodaj zmiany `.sqlx/` do PR:

```powershell
$env:DATABASE_URL = 'postgres://game:game@127.0.0.1:5433/game'
cargo run -p server --locked -- migrate
$env:SQLX_OFFLINE = 'false'
cargo sqlx prepare --workspace -- --all-targets --locked
cargo sqlx prepare --check --workspace -- --all-targets --locked
Remove-Item Env:SQLX_OFFLINE
```

W produkcji na dedykowanym klastrze administrator uruchamia `deploy/postgres/roles.sql` w bazie aplikacji i ustawia hasła ról poza Git (np. `psql` i `\password`). Następnie uruchom migracje jako `game_migrator` przez `MIGRATION_DATABASE_URL`, a po nich `deploy/postgres/privileges.sql` jako ta sama rola. Powtarzaj migracje i nadanie uprawnień przy kolejnych wdrożeniach. Serwer łączy się jako `game_app` przez `DATABASE_URL`; rola `game_readonly` służy do odczytu. `game_app` nie może zmieniać schematu ani historii migracji. Konto używane przez lokalny Docker jest uproszczeniem deweloperskim.

### Konta i lokalny gość (F6.2)

Po `run.bat` ekran „Konto” udostępnia rejestrację, logowanie i reset hasła. Lokalnie wiadomości trafiają do `target/mail/*.json`; otwórz link z pola `body`, aby potwierdzić adres lub ustawić nowe hasło. To transport deweloperski, bez wysyłania poczty. Istniejący `.env` nie jest nadpisywany: przy serwerze nasłuchującym na loopback brak `PUBLIC_ORIGIN` przyjmuje `http://localhost:5173` i lokalną pocztę. Vite przekazuje `/api` do serwera.

Przy wdrożeniu publicznym ustaw dokładny `PUBLIC_ORIGIN=https://…`, `MAIL_MODE=smtp`, `SMTP_HOST`, `SMTP_PORT` (domyślnie 587, STARTTLS), `SMTP_USER`, `SMTP_PASSWORD`, `SMTP_FROM`, `WS_TICKET_KEY` (32 losowe bajty jako 64 znaki hex), `TURNSTILE_SITE_KEY` i `TURNSTILE_SECRET`. Tylko klucz publiczny Turnstile trafia do klienta; widżet ładuje się po otwarciu rejestracji. Klucze SMTP/Turnstile pozostają po stronie serwera. Dla proxy ustaw `TRUSTED_PROXY_IPS` wyłącznie na rzeczywiste adresy zaufanych pośredników; bez tego serwer ignoruje `X-Forwarded-For`. Konfiguracja poza loopback wymaga HTTPS, SMTP i CAPTCHA. `server migrate` nie wymaga konfiguracji uwierzytelniania.

Gość jest wyłącznie stanem w pamięci frontendu (ADR-018). Nie korzysta z API, nie dostaje tokenu ani ciasteczka, nie zapisuje wyborów i postępu w pamięci trwałej przeglądarki. Odświeżenie usuwa gościa. Statyczne zasoby PWA mogą pozostać w cache do gry offline.

API Androida wymaga `client=android` przy logowaniu i `X-Native-Version: 1.0.0`; później `Authorization: Bearer …`. Do połączeń służy `/api/auth/ws-ticket`, do rotacji `/api/auth/rotate`. Przy budowaniu klienta Android podaj `VITE_API_ORIGIN` jako dokładny adres HTTPS API; trafi także do CSP. Aktualny klient przechowuje token tylko w RAM i rotuje go przed kolejnym żądaniem po 7 dniach. Trwały magazyn Android Keystore i pełny cykl życia powłoki należą do F12A. Procedurę i zakres weryfikacji opisuje [raport F6.2](docs/23-konta-i-gosc-testy.md).

### Lobby i rzeczywiste E2E (F6.3–F7.1)

`run.bat` przekazuje także WebSocket `/ws` przez Vite. Po rejestracji i zalogowaniu wybierz „Lobby”, a potem pomieszczenie. Gość nie otwiera WebSocket. Limity połączeń konfiguruje `.env.example`.

### Konta sterowane przez serwer

Po migracji `cargo run -p server --locked -- seed-bots` przygotowuje 40 trwałych kont z
`crates/server/src/bots/roster.toml`. Ponowienie zachowuje postęp i ustawienie `enabled`.
Jeśli aktywna klasa została wycofana z wydania, tworzy zatwierdzonego zastępcę z tym
samym poziomem i PD; stara postać i historia pozostają. Konflikt nicku z innym kontem wycofuje całą operację.
Hasła są losowe i nie są przechowywane w postaci jawnej ani wypisywane. Nie wysyłamy poczty
do technicznych adresów tych kont podczas tworzenia.

Ustaw `LOBBY_BOTS_ENABLED=true` w `.env` i uruchom serwer. Sterownik tworzy brakujące
konta, łączy je z lobby, przyjmuje wyzwania na arenie i okresowo zmienia miejsca części
postaci. Większość pozostaje na arenie. Zwykłe profile mają nick, postać, poziom i historię;
wewnętrzne `bot_accounts` nie jest polem protokołu klienta.

`LOBBY_BOTS_ENABLED=false` i restart wyłączają całą grupę bez usuwania danych.
Pojedyncze konto można wyłączyć przez `UPDATE bot_accounts SET enabled=false WHERE roster_key='astar'`;
działający sterownik sprawdza to co 30 sekund. `true` włącza je ponownie.
Blokada PostgreSQL pozwala działać jednemu sterownikowi na bazę; konta są obecne
w hubie tego procesu, zgodnie z obecną architekturą jednego serwera lobby.

Walki gracza z takim kontem korzystają z normalnych zasad areny i jej limitów nagród.
Pojedynki samych kont sterowanych przez serwer używają prawdziwego silnika, ale mają
rodzaj `training`, bez PD, Dukatów i zmian rankingu. Konta nie zapisują się automatycznie
do turniejów i nie generują wiadomości na czacie.

Testy: `cargo test -p server --test online bots:: -- --ignored --test-threads=1`
na lokalnym PostgreSQL. Sprawdzają ponowienie tworzenia, konflikt nazwy, pełną walkę
przez WebSocket, restart, wyłączanie, blokadę drugiego sterownika i autonomiczny pojedynek.

### Testy przegl?darkowe z backendem

`pnpm --filter e2e test` wymaga teraz lokalnego PostgreSQL z prawem `CREATEDB`. Domyślnie używa deweloperskiego klastra na 5433; opcjonalnie podaj `E2E_DATABASE_URL` (tylko loopback). Nie czyta aplikacyjnego `.env` ani `DATABASE_URL`. Playwright uruchamia przykład `browser_fixture`, który tworzy własną bazę `game_e2e_<uuid>`, stosuje migracje i przechwytuje pocztę w RAM. Backend słucha na 18092, preview na 4173. Teardown zamyka WebSocket, usuwa wyłącznie wygenerowaną bazę i czeka na zakończenie sprzątania. Zwykły serwer nie zawiera testowego endpointu `__shutdown`. Testy „rejestracja → lobby” i dwóch klientów korzystają z prawdziwego backendu, bez mocków. Pozostałe formularze nadal mają osobne testy kontraktu API.

## 3. Gałęzie, commity i PR

- Praca na krótkich gałęziach od `main`; jeden krok planu (`docs/16-plan-wdrozenia.md`) to jeden PR (duże kroki: kilka PR-ów).
- Commit: krótki opis w trybie rozkazującym z przedrostkiem `feat`, `fix`, `test`, `docs`, `refactor`, `perf` albo `chore`, np. `feat: dodaj rozstrzyganie ataków (R-ATK-01…07)`.
- PR wypełnia checklistę z szablonu (`.github/pull_request_template.md`) i wymaga zielonego CI. Zmiany w silniku i ekonomii zawsze przechodzą przegląd (`docs/14-testy-i-jakosc.md`, rozdz. 10).
- Każda reguła `R-…` ze specyfikacji ma test, który odwołuje się do jej identyfikatora.

## 4. Zasady kodu

- Crate'y silnika (`engine*`) nie używają liczb zmiennoprzecinkowych, `HashMap`/`HashSet`, czasu systemowego ani losowości spoza jawnych strumieni (`docs/06-silnik-gry.md`, rozdz. 7). Pilnuje tego clippy (`clippy.toml`, `[workspace.lints]` w `Cargo.toml`). Serwer i narzędzia luzują te zakazy we własnym `Cargo.toml`.
- Nazwy w kodzie są neutralne, bez nazwy produktu (`docs/04-architektura.md`, rozdz. 7).
- Formatowanie: `cargo fmt` (Rust), Biome (TypeScript, CSS, JSON), `buf format` (Protobuf). Zakończenia linii LF, poza skryptami Windows (`.gitattributes`).

## 5. Decyzje architektoniczne (ADR)

Rejestr decyzji to `docs/05-decyzje-architektoniczne.md`. Nowa decyzja albo zmiana istniejącej to nowy ADR z kolejnym numerem: kontekst, decyzja, rozważone alternatywy i konsekwencje, plus wiersz w tabeli na początku dokumentu. Zastąpiony ADR dostaje status „zastąpiona przez ADR-0xx”. Nowe założenia dotyczące zasad gry trafiają do rejestru w `docs/02-zasady-gry.md`, rozdz. 15.

## 6. Zależności

- Wersje są zapisane w `Cargo.lock` i `pnpm-lock.yaml`, a w `package.json` dokładnie (bez `^`).
- Aktualizacje proponuje Renovate (`renovate.json`) raz w tygodniu, w małych PR-ach z pełnym CI.
- pnpm blokuje skrypty instalacyjne zależności. Wyjątki, każdy z uzasadnieniem, są w `pnpm-workspace.yaml` (`allowBuilds`).
