# Spellgambit: Arcane Class

Projekt gry karcianej z elementami RPG (przeglądarka i Android), odtwarzającej zasady polskiej gry sieciowej **Cellmons** (Pogram, 2004–2009). Mechanika pochodzi z oryginału, a nazwy, teksty, fabuła i grafiki są własne. „Cellmons” zostaje tylko nazwą repozytorium. Nazwę produktu, znak towarowy i domeny opisuje `docs/20-nazwa-i-znak-towarowy.md`.

**Status:** dokumentacja i plan wdrożenia. Kod jeszcze nie powstał; pierwszy krok to faza F0 z `docs/16-plan-wdrozenia.md`.

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
