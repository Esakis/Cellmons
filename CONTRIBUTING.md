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

Na Windowsie Rust potrzebuje narzędzi C++ z Visual Studio (Build Tools, składnik „Desktop development with C++”).

`wasm-bindgen-cli` dostarcza `wasm-bindgen-test-runner`, który uruchamia testy silnika w WebAssembly (Node). Bez niego `check.bat` pomija krok „natywne = WASM”, ale CI wykonuje go zawsze. Po zmianie wersji crate'a `wasm-bindgen` trzeba zainstalować tę samą wersję narzędzia.

## 2. Uruchomienie i sprawdzanie zmian

| Polecenie | Działanie |
|---|---|
| `run.bat` | uruchamia bazę (PostgreSQL w Dockerze), serwer gry i klienta, potem otwiera przeglądarkę na http://localhost:5173 |
| `run.bat -NoBrowser` | to samo bez otwierania przeglądarki |
| `run.bat -Stop` | zatrzymuje bazę; dane zostają w woluminie Dockera |
| `check.bat` | wszystkie kontrole z CI: Rust, silnik do WebAssembly z testem natywne = WASM, klient, proto, testy e2e i serwer z bazą |
| `check.bat -Quick` | szybsze sprawdzenie, bez testów e2e i testu serwera z bazą |
| `cargo run -p sim -- duel [ziarno] [--a L0\|L1] [--b L0\|L1]` | pojedynek botów (domyślnie L1 przeciw L1) na losowych postaciach i kartach testowych, opisany runda po rundzie |
| `cargo run --release -p sim -- run [--n 1000] [--seed 0] [--a L1] [--b L1]` | seria pojedynków botów: wygrane, remisy, powody końca, średnia liczba rund, przewaga inicjatywy i czas |
| `cargo run -p content-compiler -- check` | sprawdza treść w `content/` i to, czy wygenerowany kod w `engine-content` jest aktualny |
| `cargo run -p content-compiler -- build` | po zmianie treści: generuje `crates/engine-content/src/generated.rs` i pliki klienta w `target/content/` |
| `cargo run -p content-compiler -- curves --weights 35,30,20,15 --school 5:combat=2` | wypisuje tabelę rozwoju postaci (poziomy 2–20) do wklejenia w `content/avatars/*.toml` |
| `cargo run --release -p sim -- matrix [--n 2000] [--level 1] [--bot L1]` | macierz wygranych postaci startowych z taliami startowymi; kod wyjścia 1, gdy któraś ma ponad 60% |
| `cargo run -p sim -- duel [ziarno] --starters dune_wanderer,ember_drake` | pojedynek postaci startowych opisany runda po rundzie |
| `cargo bench -p engine` | pomiar rozstrzygania rundy i całego pojedynku (`docs/06-silnik-gry.md`, rozdz. 10) |

- Serwer i klient działają w osobnych oknach; zamknięcie okna zatrzymuje usługę.
- W wersji deweloperskiej panel „Środowisko lokalne” w rogu strony pokazuje, czy działają serwer gry i baza.
- Konfiguracja lokalna jest w pliku `.env`, który `run.bat` tworzy z `.env.example`. Plik `.env` nie trafia do repozytorium.
- Lokalna baza używa portu 5433, a serwer gry portu 8090, bo porty domyślne (5432 i 8080) często zajmują inne programy.
- `check.bat` uruchamiaj po każdym kroku planu i przed każdym commitem. Kończy się podsumowaniem i kodem wyjścia 0 albo 1.
- Poza Windowsem kontrole można uruchomić ręcznie; pełną listę poleceń zawiera `.github/workflows/ci.yml`.

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
