# Spellgambit: Arcane Class

Projekt gry karcianej z elementami RPG (przeglądarka i Android), odtwarzającej zasady polskiej gry sieciowej **Cellmons** (Pogram, 2004–2009). Mechanika pochodzi z oryginału, a nazwy, teksty, fabuła i grafiki są własne. „Cellmons” zostaje tylko nazwą repozytorium. Nazwę produktu, znak towarowy i domeny opisuje `docs/20-nazwa-i-znak-towarowy.md`.

**Status:** faza F4 z `docs/16-plan-wdrozenia.md`: boty L0–L3 i cztery profile treningowe. L2 uczy się ujawnionych zachowań przeciwnika, a L3 rozstrzyga hipotetyczne rundy i losuje ze strategii mieszanej. Boty korzystają wyłącznie z widoku gracza i działają deterministycznie natywnie oraz w WebAssembly. `sim ai` sprawdza ich siłę i czas decyzji. Treść gry z F3 jest w `content/` (72 karty, 6 postaci startowych, zestawy startowe, teksty PL i EN), a zatwierdzone grafiki w `art/`. Następna faza, F5, to trening offline w przeglądarce.

## Uruchomienie lokalne

Potrzebne są: Rust (rustup), Node.js 24 LTS z pnpm (`corepack enable pnpm`) i Docker Desktop. Szczegóły są w [CONTRIBUTING.md](CONTRIBUTING.md).

- `run.bat` uruchamia bazę danych, serwer gry i klienta, a potem otwiera przeglądarkę na http://localhost:5173.
- `check.bat` wykonuje wszystkie kontrole z CI i kończy się podsumowaniem.
- `cargo run -p sim -- duel [ziarno]` rozgrywa pojedynek botów i opisuje go runda po rundzie.
- `cargo run --release -p sim -- run --n 100000` rozgrywa serię pojedynków botów i podsumowuje wyniki.
- `cargo run --release -p sim -- matrix` pokazuje macierz wygranych postaci startowych.
- `cargo run --release -p sim -- ai --n 1200` sprawdza hierarchię L2–L1 i L3–L2 dla botów domyślnych oraz profili z ich własnymi taliami i mierzy czas decyzji.
- `cargo run -p sim -- duel 3 --a L3 --b L2 --starters dune_wanderer,ember_drake` opisuje walkę nowych botów na prawdziwej treści gry.
- `cargo run -p content-compiler -- build` sprawdza treść z `content/` i generuje z niej kod silnika.

## Najważniejsze decyzje

| Obszar | Decyzja |
|---|---|
| Nazwa produktu | „Spellgambit: Arcane Class”; znak słowny SPELLGAMBIT do zgłoszenia w EUIPO (kl. 9 i 41) |
| Zasady | klasyczna wersja 2004–2009 (bez Eteru), talia 20 kart, waluta Dukaty |
| Treść | mechanika z oryginału, własne nazwy, teksty i grafiki |
| Płatności | brak na start; waluta zdobywana tylko w grze |
| Silnik reguł | Rust, wspólny dla serwera i przeglądarki (WebAssembly) |
| Serwer | Rust (Tokio, Axum), jedna binarka, model aktorów |
| Komunikacja | WebSocket + Protobuf |
| Klient | TypeScript + SolidJS + Vite, PWA z treningiem offline |
| Dane | PostgreSQL |
| Hosting | VPS w UE + Cloudflare, Docker, GitHub Actions |
| Aplikacja mobilna | Android przez Capacitor: Google Play (AAB) i APK do pobrania ze strony |
| Grafika | SVG tworzone przez asystenta AI, element po elemencie, zatwierdzane przez właściciela |
| Administrator danych | firma właściciela projektu |

Uzasadnienia: `docs/05-decyzje-architektoniczne.md`.

## Dokumentacja

Zalecana kolejność czytania: 01 → 02 → 03 → 04 → 16, potem pozostałe według potrzeby.

| Dokument | Zawartość |
|---|---|
| [01. Badania i źródła](docs/01-badania-i-zrodla.md) | historia gry, źródła z oceną wiarygodności, weryfikacja zasad na zapisach walk |
| [02. Zasady gry](docs/02-zasady-gry.md) | pełna specyfikacja reguł (identyfikatory `R-…`), parametry, rejestr założeń |
| [03. Treść gry](docs/03-tresc-gry.md) | szkoły, postacie, rozwój, karty, rodziny efektów, format danych |
| [04. Architektura](docs/04-architektura.md) | komponenty, przepływy, struktura repozytorium, skalowanie |
| [05. Decyzje architektoniczne](docs/05-decyzje-architektoniczne.md) | ADR z alternatywami i konsekwencjami |
| [06. Silnik gry](docs/06-silnik-gry.md) | model danych, potok rundy, system efektów, determinizm, WASM |
| [07. Protokół sieciowy](docs/07-protokol-sieciowy.md) | WebSocket + Protobuf, katalog wiadomości, HTTP API |
| [08. Serwer](docs/08-serwer.md) | moduły, aktory, pojedynki, ekonomia, turnieje, wygaszanie |
| [09. Baza danych](docs/09-baza-danych.md) | schemat, transakcje, migracje, retencja |
| [10. Klient webowy](docs/10-klient-web.md) | ekrany, ekran walki, sieć, PWA, dostępność |
| [11. AI i symulacje](docs/11-ai-i-symulacje.md) | boty L0–L3, symulator balansu, progi |
| [12. Bezpieczeństwo i prawo](docs/12-bezpieczenstwo-i-prawo.md) | zagrożenia, fair play, RODO, własność intelektualna |
| [13. Wydajność](docs/13-wydajnosc.md) | budżety, techniki, testy obciążeniowe |
| [14. Testy i jakość](docs/14-testy-i-jakosc.md) | strategia testów, test referencyjny TR-01, CI, checklista PR |
| [15. Infrastruktura i wdrożenie](docs/15-infrastruktura-i-wdrozenie.md) | środowiska, CI/CD, wdrożenia bez przerw, kopie, monitoring, procedury |
| [16. Plan wdrożenia](docs/16-plan-wdrozenia.md) | fazy F0–F15, kroki z kryteriami akceptacji, kamienie milowe |
| [17. Ryzyka i otwarte pytania](docs/17-ryzyka-i-otwarte-pytania.md) | rejestr ryzyk, decyzje do podjęcia, lista cięć |
| [18. Słownik](docs/18-slownik.md) | pojęcia PL / EN / kod |
| [19. Aplikacja Android](docs/19-aplikacja-android.md) | Capacitor, logowanie w aplikacji, aktualizacje, podpisywanie, Google Play i APK |
| [20. Nazwa i znak towarowy](docs/20-nazwa-i-znak-towarowy.md) | nazwa produktu, sprawdzenie znaków i domen, wykaz towarów i usług, kroki zgłoszenia |
| [Dane referencyjne](docs/dane/README.md) | dane z archiwum oryginalnej gry (tylko do projektowania) |
