# 13. Wydajność

Wydajność jest głównym wymaganiem projektu. Ten dokument określa budżety (liczby, które muszą być spełnione), techniki ich osiągania, sposób pomiaru i testy obciążeniowe. Budżety są sprawdzane automatycznie w CI tam, gdzie to możliwe.

## 1. Budżety

### 1.1. Klient

| Miara | Budżet | Jak mierzymy |
|---|---|---|
| JS na start (trasa logowania i mapy), po Brotli | ≤ 70 KB | kontrola rozmiaru w CI |
| CSS na start, po Brotli | ≤ 20 KB | kontrola rozmiaru w CI |
| Czcionki | ≤ 40 KB | kontrola rozmiaru w CI |
| Pierwsze załadowanie łącznie (bez ilustracji kart) | ≤ 200 KB | Lighthouse CI; kontrola rozmiaru w CI (cała aplikacja offline bez modułu ilustracji) |
| Ilustracje kart (jeden leniwie ładowany moduł `cardArt`), po Brotli | ≤ 80 KB | kontrola rozmiaru w CI |
| Pakiet ekranu walki (ładowany leniwie) | ≤ 40 KB | kontrola rozmiaru w CI |
| `engine-wasm` po Brotli | ≤ 90 KB | kontrola rozmiaru w CI |
| Dane treści (`content.<hash>.json`) po Brotli | ≤ 50 KB | kontrola rozmiaru w CI |
| LCP (4G, telefon średniej klasy) | ≤ 1,5 s | WebPageTest przed kamieniami milowymi; RUM |
| INP (75. percentyl) | ≤ 100 ms | RUM (`web-vitals`) |
| CLS | ≤ 0,05 | Lighthouse CI, RUM |
| Wynik Lighthouse (mobile) | Performance ≥ 90, Accessibility ≥ 95 | Lighthouse CI |
| Od przyjęcia wyzwania do interaktywnego ekranu walki | ≤ 300 ms | test e2e z pomiarem |
| Animacje walki | 60 FPS, brak długich zadań > 50 ms | test w Playwright ze śladem wydajności |
| Pamięć JS w trakcie walki (telefon) | ≤ 150 MB | Chrome DevTools, test ręczny przed M2 i M6 |

### 1.2. Sieć

| Miara | Budżet |
|---|---|
| `DuelSubmitMove` | ≤ 40 B |
| `DuelRoundResult` (typowo) | ≤ 600 B |
| `DuelStart` | ≤ 2 KB |
| `RoomSnapshot` dla pokoju z 500 graczami | ≤ 40 KB |
| Opóźnienie po stronie serwera: od drugiego ruchu do wysłania wyniku (p99) | ≤ 5 ms |
| Wynik rundy u gracza od wysłania drugiego ruchu (p95, gracze w UE) | ≤ 100 ms |

### 1.3. Serwer i baza (VPS 4 vCPU, 8 GB RAM)

| Miara | Budżet (cel do potwierdzenia testami w F13) |
|---|---|
| Jednoczesne połączenia | ≥ 5 000 przy CPU ≤ 60% |
| Jednoczesne pojedynki | ≥ 2 000 |
| Czat: rozsyłanie w pokoju 1000 osób | 500 wiadomości/s bez wzrostu opóźnień |
| Pamięć na połączenie (z buforami) | ≤ 64 KB |
| Pamięć na aktora pojedynku | ≤ 16 KB |
| Zapytania do bazy (p95) | ≤ 5 ms |
| Transakcja wyniku pojedynku (p95) | ≤ 15 ms |
| Zakup i otwarcie zestawu kart (p95) | ≤ 20 ms |
| Start serwera (bez odtwarzania pojedynków) | ≤ 5 s |

Budżety silnika i AI: `06-silnik-gry.md`, rozdz. 10 oraz `11-ai-i-symulacje.md`, rozdz. 7. Budżety aplikacji Android (rozmiar APK, zimny start, pamięć, bateria): `19-aplikacja-android.md`, rozdz. 10.

## 2. Techniki

### 2.1. Klient

- Podział kodu według tras. Ekran walki i worker z WASM ładowane leniwie, ale **podgrzewane** przy otwarciu okna wyzwania albo przy wejściu do sali treningowej (`<link rel="modulepreload">`, wczesne `fetch` WASM).
- SolidJS bez wirtualnego DOM. Brak dużych bibliotek; każda zależność sprawdzana pod kątem rozmiaru.
- Krytyczny CSS w `index.html`, reszta w plikach CSS według tras.
- Animacje wyłącznie na `transform` i `opacity`. Bez odczytów układu w pętli animacji.
- Ilustracje: AVIF i WebP w dwóch rozmiarach, `loading="lazy"`, wymiary zadeklarowane z góry (brak skoków układu).
- Ikony jako jeden sprite SVG.
- Czcionka zmienna z podzbiorem znaków (łacina z polskimi znakami) albo czcionki systemowe.
- Service Worker z precache szkieletu aplikacji: drugie i kolejne wejścia praktycznie bez sieci.
- Pliki z hashem w nazwie i nagłówek `Cache-Control: public, max-age=31536000, immutable`; `index.html` bez cache.

### 2.2. Sieć

- Jedno połączenie WebSocket, binarne wiadomości Protobuf, bez kompresji (narzut CPU większy niż zysk przy małych wiadomościach).
- Cloudflare: HTTP/3 i Brotli dla zasobów statycznych, zasoby z cache brzegowego.
- Serwer w UE (ok. 20–30 ms od Polski).

### 2.3. Serwer

- Rust bez GC; Tokio; `mimalloc`.
- Aktory bez blokad w gorącej ścieżce. Rejestry na sharded mapach.
- Rozsyłanie bez kopiowania: wiadomość kodowana raz do `Bytes` i współdzielona przez wszystkich odbiorców.
- Ponownie używane bufory kodowania Protobuf.
- Zapisy niekrytyczne (dziennik rund, dziennik czatu) wykonywane asynchronicznie i paczkami (np. co 100 ms lub co 100 wpisów).
- AI L3 w osobnej puli wątków z limitem czasu.
- Profil `release`: `lto = "fat"`, `codegen-units = 1`, kompilacja pod docelowy procesor (`target-cpu` bazowy dla x86-64-v2).

### 2.4. Baza

- Zapytania po kluczach głównych i indeksach. Plan każdego nowego zapytania sprawdzany przez `EXPLAIN ANALYZE` na danych testowych.
- Statystyki efektywne postaci liczone w pamięci, nie w SQL.
- Ranking z indeksu i cache w pamięci (odświeżany co minutę).
- Partycjonowanie `matches`. Krótkie transakcje bez oczekiwania na sieć w trakcie.
- Podstawowe strojenie PostgreSQL (`shared_buffers` ok. 25% RAM, `effective_cache_size`, `work_mem`, `max_connections` dopasowane do puli).

## 3. Pomiary i narzędzia

| Obszar | Narzędzia |
|---|---|
| Klient | Lighthouse CI, WebPageTest, Chrome DevTools (Performance, Memory), `rollup-plugin-visualizer`, `web-vitals` (RUM) |
| Silnik i AI | `criterion` (lokalnie), `iai-callgrind` (liczba instrukcji, stabilna w CI), benchmark WASM w Playwright |
| Serwer | metryki Prometheus (histogramy opóźnień), `tracing` (spany), `tokio-console` (lokalnie), `cargo flamegraph` / `perf`, profilowanie pamięci (`dhat`, `heaptrack`) |
| Baza | `pg_stat_statements`, `auto_explain` dla zapytań > 50 ms, metryki postgres-exporter |

## 4. Bramki w CI

| Bramka | Kiedy | Warunek |
|---|---|---|
| Rozmiar pakietów i WASM | każdy PR (zmiany w `web` lub `engine*`) | budżety z rozdz. 1.1 |
| Lighthouse CI | każdy PR z zmianami w `web` (build podglądowy) | Performance ≥ 90, Accessibility ≥ 95 |
| Regresja silnika | każdy PR z zmianami w `engine*` | wzrost liczby instrukcji `resolve_round` > 10% blokuje PR |
| Test dymny obciążenia | co noc na środowisku testowym | scenariusz S2 w skali 10%, budżety opóźnień spełnione |
| Pełne testy obciążeniowe | przed M3, M4, M6 i przed dużymi wydaniami | wszystkie scenariusze z rozdz. 5 |

## 5. Testy obciążeniowe

**Narzędzie:** crate `loadtest`, czyli boty w Rust używające prawdziwego protokołu (crate `protocol`) i botów L0/L1 do wyboru ruchów. Konfiguracja scenariuszy w TOML. Wynik: histogramy opóźnień (HDR), liczba błędów, przepustowość. Endpointy HTTP (logowanie, rejestracja, strony publiczne) testuje k6.

Generator obciążenia działa na osobnej maszynie w tym samym regionie, żeby nie konkurował z serwerem o zasoby.

| Scenariusz | Opis | Kryterium |
|---|---|---|
| S1 Lobby | 5000 graczy w pokojach, czat 1 wiadomość na gracza na minutę, 5% wymiany obecności na minutę | p95 dostarczenia czatu ≤ 100 ms |
| S2 Pojedynki | 2000 jednoczesnych pojedynków, czas namysłu botów 2–20 s | budżety z rozdz. 1.2 |
| S3 Szczyt turniejowy | 10 turniejów po 64 graczy startuje jednocześnie | brak błędów, opóźnienia w budżecie |
| S4 Burza połączeń | 50% połączeń zrywa się i wraca w ciągu 10 s (symulacja awarii sieci lub wdrożenia) | wszyscy wznawiają sesje, żaden pojedynek nie jest przerwany |
| S5 Ekonomia | 200 równoległych zakupów, otwarć i wymian | po teście saldo = suma księgi, brak duplikacji |
| S6 Długi test | 24 h przy 50% obciążenia S1 + S2 | wzrost pamięci < 5%, brak degradacji opóźnień |

Ogólne kryteria: odsetek błędów < 0,1%, CPU ≤ 60% przy docelowym obciążeniu.

## 6. Pojemność i progi skalowania

- Wyniki S1–S2 wyznaczają **przetestowaną pojemność** jednej instancji.
- Alerty (`15-infrastruktura-i-wdrozenie.md`, rozdz. 8): CPU > 60% przez 15 min, liczba połączeń > 70% przetestowanej pojemności, p95 wyniku rundy > 100 ms.
- Pierwszy krok przy zbliżaniu się do progów: większy VPS (skalowanie pionowe jest najprostsze). Kolejny: etap 2 z `04-architektura.md`, rozdz. 11.

## Pomiar F6.3–F7.1

Test integracyjny `f63_thousand_authenticated_connections_and_binary_pong` utrzymuje 1000 równoczesnych, uwierzytelnionych połączeń na prawdziwym HTTP/WebSocket/PostgreSQL i weryfikuje skorelowany Protobuf Pong dla każdego. Lokalny przebieg debug: około 4,3 s na utworzenie kont testowych, otwarcie połączeń i odpowiedzi. To test przyjęcia połączeń; nie stanowi pomiaru 1000 aktywnych rozmów ani limitu produkcyjnego sprzętu.

Budżet 200 KiB Brotli dotyczy całego pakietu treningu offline bez ilustracji kart. Ekrany konta i lobby wymagają sieci i są osobnymi zasobami na żądanie (limit 45 KiB). Lista `dist/online-assets.json` powstaje z grafu manifestu Vite; współdzielone zależności nadal wliczają się do offline. Service Worker nie zapisuje API, WebSocket ani danych użytkownika. Build kontroluje obie grupy bez podnoszenia limitu 200 KiB.

Wynik po zmianach F6.3–F7.1 i równoległej rozbudowie ekranu treningu: 199,9 / 200 KiB offline, 14,5 / 45 KiB konta/lobby na żądanie. Vite skraca nazwy wynikowe CSS Modules do wspólnego prefiksu pliku i lokalnego indeksu; opisowe nazwy w kodzie pozostają czytelne.
