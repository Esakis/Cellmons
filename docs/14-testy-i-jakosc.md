# 14. Testy i jakość

Testy są częścią definicji ukończenia każdego kroku planu (`16-plan-wdrozenia.md`). Ten dokument opisuje rodzaje testów w każdej warstwie, test referencyjny TR-01, potok CI i checklistę PR.

## 1. Zasady

- **Śledzalność reguł:** każda reguła `R-…` z `02-zasady-gry.md` ma co najmniej jeden test, który odwołuje się do jej identyfikatora w nazwie albo w atrybucie. Skrypt w CI wypisuje reguły bez testów i blokuje PR, jeśli zmieniona reguła nie ma pokrycia.
- **Piramida testów:** dużo szybkich testów silnika i usług, mniej testów integracyjnych, kilkanaście scenariuszy end-to-end.
- **Determinizm testów:** testy losowe używają stałych ziaren, a przy błędzie wypisują ziarno, żeby dało się go odtworzyć.
- **Nie osłabiamy testów, żeby przeszły.** Czerwony test to błąd do naprawienia albo świadoma zmiana zasad z aktualizacją specyfikacji.

## 2. Silnik

| Rodzaj | Narzędzie | Zakres |
|---|---|---|
| Jednostkowe | `cargo test` | każda reguła R-POJ, R-RUN, R-ATK, R-CZA, R-KON |
| Scenariuszowe (złote) | konstruktor scenariuszy + `insta` (migawki zdarzeń) | każda karta: przypadek podstawowy + co najmniej jeden brzegowy |
| Właściwościowe | `proptest` | niezmienniki (rozdz. 2.1) na losowych stanach i ruchach |
| Fuzzing | `cargo-fuzz` | dekodowanie ruchów i powtórek, `resolve_round` na losowych danych: brak paniki |
| Natywne = WASM | `wasm-bindgen-test` w Node | te same ziarna dają ten sam `state_hash` po każdej rundzie |
| Regresja powtórek | korpus powtórek (z testów, z bety, z produkcji) | odtworzenie daje zapisany wynik |
| Mutacyjne | `cargo-mutants` (co noc) | odsetek przeżywających mutantów w `engine` < 10% |
| Wydajnościowe | `criterion`, `iai-callgrind` | budżety z `06-silnik-gry.md`, rozdz. 10 |
| Test referencyjny | TR-01 | rozdz. 3 |

### 2.1. Niezmienniki (testy właściwościowe)

1. `W ≥ 0` i `K ≥ 0` po każdej operacji.
2. Statystyki efektywne ≥ 0, a `hp_max` i `stamina_max` ≥ 1 (Z-ENG-2).
3. Ręka ≤ 5 kart; liczba kart w ręce, talii i na stosie odrzuconych jest stała (poza efektami jawnie tworzącymi lub niszczącymi karty).
4. Liczba rund ≤ 5; po `Finished` żadna operacja nie zmienia stanu.
5. Determinizm: dwa wywołania z tym samym wejściem dają identyczny stan i zdarzenia.
6. Symetria: zamiana graczy miejscami (wraz z inicjatywą) daje lustrzany wynik.
7. Ukryte informacje: `view(state, A)` nie zawiera ręki B, kolejności talii ani ziarna.
8. Poprawny ruch z `validate_move` nigdy nie powoduje błędu w `resolve_round`.
9. Koszt K ruchu = liczba zmienionych slotów × koszt zmiany + koszt K czaru (R-RUN-05).

## 3. Test referencyjny TR-01

Odtworzenie walki #26519 z archiwum (`01-badania-i-zrodla.md`, rozdz. 5.2). Sprawdza rdzeń walki na prawdziwych danych.

**Ustawienia:**
- Gracz A: AF 4, OF 2, AM 9, OM 3, Ż 75, W 74. Domyślnie ataki M M M, obrony M M M.
- Gracz B: AF 8, OF 3, AM 7, OM 4, Ż 66, W 101. Domyślnie ataki F F F, obrony M M M.
- Ręka B zawiera dwie karty testowe „bez efektu” (K 2, W 8) i kartę testową „leczenie 21” (K 2, W 8).
- Inicjatywa: dowolna (nie wpływa na rundy 1–3).

| Runda | Ruch A (ataki / obrony / karta) | Ruch B (ataki / obrony / karta) | Oczekiwane po rundzie: A (Ż / W / K) | B (Ż / W / K) |
|---|---|---|---|---|
| 1 | M F F / M M F / — | F F F / M M M / bez efektu | 55 / 66 / 1 | 60 / 77 / 2 |
| 2 | F M F / M F F / — | F F F / M F F / bez efektu | 49 / 53 / 2 | 45 / 61 / 2 |
| 3 | M M M / M F M / — | F F F / F M F / leczenie 21 | 37 / 35 / 3 | 56 / 37 / 2 |

Wszystkie 18 wartości muszą się zgadzać. Test jest częścią kryteriów kroku F2.5.

## 4. Treść gry

- Kompilator treści odrzuca błędne dane (`03-tresc-gry.md`, rozdz. 8.3).
- **Pokrycie kart:** skrypt porównuje listę kluczy kart z listą testów scenariuszowych. Karta bez testu blokuje PR.
- **Teksty:** każdy klucz istnieje w `pl.ftl` i `en.ftl`, zmienne w tekście zgadzają się z parametrami efektu.
- **Balans (dymny):** dla PR-ów zmieniających treść CI uruchamia skrócone `sim matrix` i `sim card` i dołącza raport (progi z `11-ai-i-symulacje.md`, rozdz. 6.2).

## 5. Serwer

| Rodzaj | Zakres | Uwagi |
|---|---|---|
| Jednostkowe | usługi domenowe z atrapami repozytoriów | szybkie, bez bazy |
| Integracyjne z bazą | repozytoria, transakcje, migracje | prawdziwy PostgreSQL w kontenerze; każdy test na osobnym schemacie |
| WebSocket | serwer uruchomiony w teście + klienci testowi (crate `protocol`) | wyzwania, pojedynki, czat, wymiany |
| Kontrakt protokołu | `buf breaking` względem gałęzi głównej; migawki binarne przykładowych wiadomości | wykrywa niezgodne zmiany schematu |
| Współbieżność | 100 równoległych prób wydania tych samych Dukatów i wymiany tych samych kart | brak duplikacji, brak ujemnych stanów, saldo = suma księgi |
| Chaos | zabicie aktora pojedynku; restart serwera w trakcie walki | pojedynek odtworzony z dziennika i dokończony |
| Uprawnienia | macierz: każda wiadomość i endpoint × rola (gość, gracz, mentor, moderator, admin) | gość nie może handlować ani grać w turniejach (R-KNT-02) |
| Limity | przekroczenie limitów wiadomości, czatu i logowania | właściwe kody błędów |

## 6. Klient

| Rodzaj | Narzędzie | Zakres |
|---|---|---|
| Jednostkowe | Vitest | warstwa sieciowa (korelacja, wznawianie), magazyny, formatowanie, komendy czatu |
| Komponentów | Vitest + Solid Testing Library | sterowanie walką (koszt K na żywo), konstruktor talii (limity), okno wymiany (reset akceptacji) |
| End-to-end | Playwright (Chromium, Firefox, WebKit; widok telefonu i komputera) | scenariusze z rozdz. 7 |
| Wizualne | zrzuty ekranu Playwright z progiem różnic | kluczowe ekrany w obu motywach i językach |
| Dostępność | `@axe-core/playwright` | brak naruszeń krytycznych i poważnych |
| Wydajność | Lighthouse CI | budżety z `13-wydajnosc.md` |
| Aplikacja Android | Maestro albo Appium na emulatorze; ręcznie na macierzy urządzeń | scenariusze i testy aktualizacji paczek z `19-aplikacja-android.md`, rozdz. 11 |

## 7. Scenariusze end-to-end

| ID | Scenariusz |
|---|---|
| E1 | rejestracja → potwierdzenie e-maila (skrzynka testowa) → lobby |
| E2 | trening offline z botem do końca pojedynku (bez sieci) |
| E3 | dwie przeglądarki: wyzwanie → pełna walka PvP → zmiana rankingu i PD |
| E4 | rozłączenie w trakcie walki → wznowienie → dokończenie |
| E5 | budowa talii → walka z użyciem czarów |
| E6 | zakup i otwarcie zestawu kart → karty w kolekcji |
| E7 | wymiana między dwoma graczami z resetem akceptacji przy zmianie oferty |
| E8 | turniej 4 graczy (gracz + boty testowe) do wypłaty nagrody |
| E9 | wyprawa „Spacer” do bossa |
| E10 | założenie klanu (5 kont) i czat klanowy |
| E11 | eksport danych i usunięcie konta |
| E12 | aplikacja Android: instalacja APK → trening offline → logowanie → walka z botem online → wyjście w tło i powrót w trakcie walki |

## 8. Potok CI

| Etap | Zawartość | Kiedy |
|---|---|---|
| Statyczna analiza | `cargo fmt --check`, `clippy -D warnings`, `biome check`, `tsc --noEmit`, `buf lint`, `buf breaking` | każdy PR |
| Testy Rust | jednostkowe, scenariuszowe, właściwościowe (skrócone), integracyjne z bazą | każdy PR (filtry ścieżek) |
| Testy WASM | budowa `engine-wasm`, natywne = WASM, rozmiar WASM | zmiany w `engine*` |
| Testy web | Vitest, budowa, rozmiar pakietów, Lighthouse CI | zmiany w `web` |
| E2E | Playwright na środowisku z docker compose | każdy PR do gałęzi głównej |
| Bezpieczeństwo | `cargo-deny`, `cargo-audit`, `pnpm audit` | każdy PR + co noc |
| Android | budowa APK i AAB, test E12 na emulatorze, kontrola rozmiaru APK | zmiany w `mobile/` lub `web/` + co noc |
| Nocne | fuzzing (30 min na cel), testy mutacyjne, pełne symulacje balansu, test dymny obciążenia | co noc |

Cel: CI dla PR-a ≤ 15 minut (cache zależności i kompilacji, równoległe joby).

## 9. Definicja ukończenia i checklista PR

Krok planu albo PR jest ukończony, gdy:
- [ ] kod spełnia kryteria akceptacji kroku z `16-plan-wdrozenia.md`;
- [ ] testy dla nowych i zmienionych reguł (z identyfikatorami `R-…`), dla każdej nowej karty test scenariuszowy;
- [ ] CI zielone, budżety wydajności spełnione;
- [ ] własny przegląd zmian (diff) przed oddaniem: brak logów debugowych, martwego kodu i zmian spoza zakresu;
- [ ] dokumentacja zaktualizowana (specyfikacja, ADR, protokół), jeśli zmienia się zachowanie;
- [ ] nowe założenia projektowe wpisane do rejestru (`02-zasady-gry.md`, rozdz. 15);
- [ ] migracje bazy w trybie expand/contract, jeśli zmienia się schemat;
- [ ] brak sekretów i danych osobowych w kodzie i logach.

## 10. Praca z repozytorium

- **Gałęzie:** praca na gałęzi głównej przez krótkie gałęzie funkcji (najwyżej kilka dni) i PR-y.
- **Commity:** krótki opis w trybie rozkazującym. Rodzaj zmiany jako przedrostek (`feat`, `fix`, `test`, `docs`, `refactor`, `perf`, `chore`).
- **Przegląd:** każdy PR przeglądany przed scaleniem (druga osoba albo sesja przeglądu z checklistą). Zmiany w silniku i ekonomii zawsze z przeglądem.
- **Wydania:** tagi `vX.Y.Z` dla serwera i klienta, osobno wersja protokołu i `content_version`. Lista zmian generowana z commitów.

## 11. Zgłaszanie błędów

Szablon zgłoszenia: kroki do odtworzenia, oczekiwane i faktyczne zachowanie, wersja klienta, identyfikator walki (powtórka) albo nagranie sesji ze środowiska testowego. Błąd w zasadach zaczyna się od testu, który go odtwarza, a dopiero potem jest naprawiany.
