# 05. Decyzje architektoniczne (ADR)

Każda decyzja ma status, kontekst, rozważone alternatywy i konsekwencje. Zmiana decyzji wymaga nowego ADR, który zastępuje stary (status „zastąpiona przez ADR-0xx”). Kryterium nadrzędne, zgodnie z założeniem projektu: **jak największa szybkość i efektywność, bez przywiązania do konkretnego języka**.

| ADR | Decyzja | Status |
|---|---|---|
| 001 | Silnik reguł w Rust, wspólny dla serwera i przeglądarki (WebAssembly) | przyjęta |
| 002 | Serwer w Rust (Tokio + Axum), modularny monolit, model aktorów | przyjęta |
| 003 | Protokół: WebSocket + Protobuf; HTTP/JSON pomocniczo | przyjęta |
| 004 | Klient: TypeScript + SolidJS + Vite; renderowanie DOM/CSS | przyjęta |
| 005 | PostgreSQL jako jedyna baza; bez Redisa na starcie | przyjęta |
| 006 | Treść jako dane; efekty kart: rodziny sparametryzowane + kod Rust, bez interpretera skryptów | przyjęta |
| 007 | Determinizm: ChaCha8 z ziarnem pojedynku, powtórka = ziarno + ruchy, zobowiązanie do ziarna | przyjęta |
| 008 | Hosting: jeden serwer w UE + Cloudflare, Docker, bez Kubernetes na start | przyjęta |
| 009 | Monorepo: workspace Cargo + pnpm | przyjęta |
| 010 | Sesje w ciasteczku HttpOnly, WebSocket w tej samej domenie z kontrolą Origin, Argon2id | przyjęta |
| 011 | Brak płatności za prawdziwe pieniądze na starcie | przyjęta (decyzja właściciela) |
| 012 | Mechanika z oryginału, treść własna (nazwy, teksty, grafiki) | przyjęta (decyzja właściciela) |
| 013 | Wersja zasad: klasyczna 2004–2009 (bez Eteru) | przyjęta (decyzja właściciela) |
| 014 | Aplikacja Android: Capacitor opakowujący klienta web; Google Play (AAB) + APK ze strony | przyjęta (decyzja właściciela) |
| 015 | Logowanie w aplikacji: token sesji + jednorazowy bilet WebSocket (uzupełnia ADR-010) | przyjęta |
| 016 | Grafika wektorowa (SVG) tworzona przez asystenta AI, element po elemencie | przyjęta (decyzja właściciela) |
| 017 | Trening offline: sesja WASM z binarnym dispatch, treść wersjonowana razem z klientem | przyjęta |
| 018 | Gość lokalny bez konta, sieci i zapisu danych; osobno konto z niepotwierdzonym e-mailem | przyjęta (decyzja właściciela) |

---

## ADR-001. Silnik reguł w Rust, wspólny dla serwera i przeglądarki

**Kontekst.** Zasady muszą działać w trzech miejscach: na serwerze (arbiter), w przeglądarce (trening offline z AI, podgląd kosztów, odtwarzanie powtórek) i w narzędziach (symulator balansu, AI, testy obciążeniowe). Muszą być deterministyczne (powtórki, testy) i bardzo szybkie, bo AI symuluje tysiące rund na jedną decyzję.

**Decyzja.** Silnik powstaje jako biblioteka Rust (`crates/engine` i `engine-cards`) bez wejścia/wyjścia, czasu systemowego i wątków. Kompiluje się natywnie (serwer, narzędzia) i do WebAssembly (przeglądarka, w Web Workerze).

**Alternatywy:**

| Opcja | Za | Przeciw |
|---|---|---|
| TypeScript wspólny dla klienta i serwera (Node/Bun) | jeden język w całym projekcie | kilka–kilkanaście razy wolniejszy w symulacjach, przerwy na GC, liczby zmiennoprzecinkowe utrudniają determinizm |
| C++ → WASM (Emscripten) | bardzo szybki | brak bezpieczeństwa pamięci, gorsze narzędzia, większy WASM |
| Go | prosty, szybki serwer | WASM rzędu 2 MB+, GC |
| C# (.NET, Blazor) | dojrzały ekosystem | ciężki runtime w przeglądarce |
| Dwie implementacje (TS w kliencie, Rust na serwerze) | każda warstwa w „swoim” języku | ryzyko rozjazdu zasad, podwójna praca |

**Konsekwencje.**
- (+) Jeden kod zasad, deterministyczny, testowany raz. Rozstrzygnięcie rundy w mikrosekundach, co daje silne AI i tanie symulacje.
- (+) Trening offline bez kosztów serwera.
- (−) Dwa języki w projekcie (Rust i TypeScript) i potok budowania WASM.
- (−) Rozmiar WASM wymaga budżetu i optymalizacji (`13-wydajnosc.md`).

## ADR-002. Serwer: Rust (Tokio + Axum), modularny monolit, model aktorów

**Kontekst.** Serwer utrzymuje długie połączenia WebSocket, prowadzi tysiące równoległych pojedynków z limitami czasu, rozsyła czat i obecność. Silnik jest w Rust (ADR-001).

**Decyzja.** Jedna binarka Rust: Tokio (asynchroniczny runtime), Axum (HTTP i WebSocket), tower (middleware). Domena podzielona na moduły. Stan współbieżny w aktorach: zadaniach Tokio z własnym stanem i kanałem wiadomości.

**Alternatywy:**

| Opcja | Za | Przeciw |
|---|---|---|
| Elixir / Phoenix | świetna obsługa obecności, kanałów, odporność na awarie | silnik przez NIF (Rustler) lub reimplementacja; drugi ekosystem |
| Go | prosta współbieżność | silnik przez cgo albo przepisanie; GC |
| Node.js / Bun | jeden język z klientem | wydajność i GC przy tysiącach pojedynków |
| Nakama (gotowy serwer gier) | gotowe konta, czat, rankingi, turnieje | logika gry w Go/Lua/TS (bez wspólnego silnika Rust), cięższa operacyjnie, narzuca protokół |
| Mikroserwisy | niezależne skalowanie | złożoność nieuzasadniona przy tej skali |

**Konsekwencje.**
- (+) Brak przerw na GC i przewidywalne opóźnienia. Mało pamięci na połączenie, wspólne typy z silnikiem.
- (+) Jeden proces do wdrożenia i monitorowania.
- (−) Konta, czat, pokoje i turnieje piszemy sami (większy zakres, pełna kontrola).
- (−) Dłuższa kompilacja Rust; wymaga dyscypliny w kodzie asynchronicznym (bez blokowania wątków runtime'u).

## ADR-003. Protokół: WebSocket + Protobuf; HTTP/JSON pomocniczo

**Kontekst.** Gra jest turowa, ale wymaga natychmiastowych powiadomień serwera (wyzwania, czat, wyniki rund). Wiadomości są małe i częste. Klient (TS) i serwer (Rust) potrzebują wspólnych, wersjonowanych typów.

**Decyzja.**
- Jedno połączenie WebSocket na kartę przeglądarki, ramki binarne Protobuf (proto3).
- Schematy w `proto/` to źródło prawdy; kod generują `prost` (Rust) i `@bufbuild/protobuf` (TS).
- HTTP/JSON tylko do logowania i rejestracji, publicznych stron (ranking, profil, powtórka walki) i webhooków.

**Alternatywy:**

| Opcja | Za | Przeciw |
|---|---|---|
| JSON przez WebSocket | prostota, czytelność | większe wiadomości, wolniejsze parsowanie, typy ręcznie |
| MessagePack | binarny, bez schematu | brak schematu i generowania typów |
| FlatBuffers / Cap'n Proto | zero-copy | niewygodne API, zysk niezauważalny przy tej wielkości wiadomości |
| WebTransport (QUIC) | nowoczesny, bez blokowania kolejki | niepotrzebny w grze turowej, nierówne wsparcie przeglądarek i proxy |
| gRPC-web / Connect | dobre dla żądanie–odpowiedź | brak dwukierunkowego strumienia w przeglądarce |

**Konsekwencje.**
- (+) Małe wiadomości (zwykle < 300 B), typy generowane, jasne zasady ewolucji schematu.
- (−) Gorsza czytelność w narzędziach przeglądarki. Dlatego w buildach deweloperskich jest inspektor protokołu, który pokazuje zdekodowane wiadomości jako JSON (`10-klient-web.md`).

## ADR-004. Klient: TypeScript + SolidJS + Vite; renderowanie DOM/CSS

**Kontekst.** Interfejs to głównie ekrany „aplikacyjne” (lobby, talie, kolekcja, sklep, czat) i ekran walki z prostymi animacjami 2D. Priorytet to szybkie ładowanie i responsywność na słabszych telefonach.

**Decyzja.** TypeScript w trybie strict i SolidJS (reaktywność drobnoziarnista, bez wirtualnego DOM, bardzo mały runtime). Budowanie przez Vite. Style w CSS Modules z tokenami projektu. Animacje przez CSS i Web Animations API. Opcjonalna warstwa Canvas/WebGL (np. PixiJS) tylko wtedy, gdy efekty tego wymagają, i ładowana leniwie.

**Alternatywy:**

| Opcja | Za | Przeciw |
|---|---|---|
| React | największy ekosystem | większy runtime, narzut wirtualnego DOM |
| Svelte 5 | mały, szybki, dobry DX | blisko Solida; wybór Solida ze względu na wydajność runtime |
| Preact | bardzo mały | wirtualny DOM |
| Vanilla TS | maksymalna wydajność | wolny rozwój, własny system komponentów |
| Rust w przeglądarce (Leptos, Yew, Dioxus) | jeden język z silnikiem | większy rozmiar startowy, wolniejszy start, mniejszy ekosystem UI |
| Silnik gier (Phaser, PixiJS) jako całe UI | dobre animacje | słaba obsługa tekstu, formularzy i dostępności |
| Unity / Godot (eksport web) | narzędzia graficzne | pobieranie rzędu megabajtów, wolny start, słaba integracja z DOM |

**Konsekwencje.**
- (+) Mały pakiet startowy i szybki DOM. Dostępność i formularze za darmo z HTML.
- (−) Mniejszy ekosystem komponentów niż React; budujemy własny mały zestaw komponentów.

## ADR-005. PostgreSQL jako jedyna baza; bez Redisa na starcie

**Kontekst.** Ekonomia (Dukaty, karty, wymiany) wymaga transakcji i więzów integralności. Obecność i pokoje są w pamięci serwera (ADR-002).

**Decyzja.** PostgreSQL dla wszystkich danych trwałych. Dostęp przez `sqlx` z zapytaniami sprawdzanymi przy kompilacji. Redis lub NATS dopiero na etapie 2 skalowania (`04-architektura.md`, rozdz. 11).

**Alternatywy:**

| Opcja | Za | Przeciw |
|---|---|---|
| SQLite (+ Litestream) | najprostsza i bardzo szybka na jednej maszynie | trudniejsze przejście na wiele instancji, jeden zapisujący |
| PostgreSQL + Redis od początku | cache i pub/sub gotowe na skalowanie | dodatkowy komponent bez potrzeby na etapie 1 |
| MongoDB | elastyczny schemat | słabsze więzy integralności dla ekonomii |
| CockroachDB / rozproszone SQL | skalowanie poziome | koszt i złożoność nieuzasadnione |

**Konsekwencje.**
- (+) Jedna technologia danych, dojrzałe narzędzia kopii i odtwarzania (PITR).
- (−) Konfiguracja kopii i monitoringu bazy jest po naszej stronie (`15-infrastruktura-i-wdrozenie.md`).

## ADR-006. Treść jako dane; efekty kart jako rodziny sparametryzowane + kod Rust

**Kontekst.** Około 330 kart o bardzo różnym działaniu. Większość to warianty kilkunastu wzorców (zmiana statystyk, leczenie, wysysanie, wymuszenie trafienia), ale część jest unikalna.

**Decyzja.**
- Metadane kart (koszty, wymagania, rzadkość) w TOML.
- Efekt karty to albo **rodzina sparametryzowana** zaimplementowana raz w Rust (np. `stat_mod`, `drain`, `enchant`), albo **efekt `custom`** napisany jako funkcja Rust z API kontekstu efektu.
- Kompilator treści generuje kod Rust z tablicami treści.
- Brak interpretera skryptów w runtime.

**Alternatywy:**

| Opcja | Za | Przeciw |
|---|---|---|
| Skrypty (Lua, Rhai, JS) | elastyczność, przeładowanie na gorąco | wolniejsze, trudniejszy determinizm i sandbox, większy WASM |
| Czysty DSL danych (JSON-logic) | wszystko w danych | nieczytelny dla skomplikowanych kart |
| Każda karta osobno w Rust | pełna kontrola | ok. 330 funkcji z powtarzalnym kodem |

**Konsekwencje.**
- (+) Około 80% kart dodaje się bez programowania. Wszystko sprawdzane przez kompilator i typy, maksymalna wydajność.
- (−) Zmiana treści wymaga wdrożenia, co jest akceptowalne i upraszcza determinizm.

## ADR-007. Determinizm i losowość

**Kontekst.** Powtórki, testy regresji, odtwarzanie pojedynków po restarcie serwera i uczciwość wymagają pełnej powtarzalności rozstrzygnięć.

**Decyzja.**
- Generator ChaCha8 z 256-bitowym ziarnem pojedynku, wylosowanym z systemowego źródła entropii.
- Osobne strumienie dla różnych celów (tasowanie talii gracza 1 i 2, efekty losowe), wyprowadzane z ziarna i etykiety. Dodanie nowego użycia losowości nie zmienia istniejących strumieni.
- Silnik używa tylko liczb całkowitych. Nie zależy od kolejności iteracji map haszujących (`Vec`, `BTreeMap`).
- Powtórka = `engine_version` + `content_version` + konfiguracja + snapshoty postaci + ziarno + ruchy.
- **Zobowiązanie do ziarna:** na starcie pojedynku obaj gracze dostają skrót `BLAKE3(ziarno)`, a ziarno ujawnia się po zakończeniu. Każdy może sprawdzić, że losowanie nie było zmieniane w trakcie.

**Alternatywy:**
- Losowanie z systemowego źródła przy każdym użyciu: brak powtórek, odrzucone.
- Wspólne ziarno z udziałem klientów (commit–reveal obu stron): niepotrzebne przy serwerze-arbitrze. Zobowiązanie do ziarna daje podobną przejrzystość mniejszym kosztem.

**Konsekwencje.**
- (+) Testy oparte na powtórkach, wykrywanie regresji, odtwarzanie walk po awarii.
- (−) Dyscyplina w kodzie silnika, pilnowana lintami i testem porównującym wyniki natywne z WASM.

## ADR-008. Hosting

**Kontekst.** Budżet niewielki, skala początkowa nieznana, gracze głównie z Polski.

**Decyzja.**
- Jeden VPS w UE (np. Hetzner, Niemcy lub Finlandia; ok. 20–30 ms od Polski).
- Docker Compose z usługami: Caddy, serwer gry (dwa sloty blue/green), PostgreSQL.
- Cloudflare przed serwerem (TLS, CDN, ochrona DDoS, WebSockety).
- CI/CD w GitHub Actions, obrazy w GHCR.
- Kopie bazy do magazynu obiektowego (S3).

**Alternatywy:**

| Opcja | Za | Przeciw |
|---|---|---|
| Kubernetes | standard dużych systemów | złożoność nieuzasadniona dla jednej binarki |
| Fly.io / Render / Railway | mniej administracji, łatwa globalizacja | wyższy koszt przy stałym obciążeniu, mniejsza kontrola |
| Serverless (Lambda) | brak serwerów | nie nadaje się do długich połączeń WebSocket i stanu w pamięci |
| Cloudflare Durable Objects | WebSocket i WASM na brzegu sieci | uzależnienie od dostawcy, baza daleko od logiki, inny model kosztów |

**Konsekwencje.**
- (+) Niski koszt (rząd kilkudziesięciu euro miesięcznie na start) i pełna kontrola.
- (−) Odpowiedzialność za aktualizacje systemu i kopie zapasowe. Pojedynczy punkt awarii, łagodzony monitoringiem, szybkim odtwarzaniem i procedurami (`15-infrastruktura-i-wdrozenie.md`).

## ADR-009. Monorepo

**Decyzja.** Jedno repozytorium: workspace Cargo (crate'y Rust) i workspace pnpm (`web`, `e2e`). CI uruchamia joby zależnie od zmienionych ścieżek.

**Uzasadnienie.** Zmiany protokołu, silnika i klienta często idą razem. Jedno repo to jeden PR i jeden przegląd.

**Alternatywa:** osobne repozytoria. Odrzucone ze względu na synchronizację wersji protokołu i silnika.

## ADR-010. Uwierzytelnianie

**Decyzja.**
- Logowanie loginem lub e-mailem i hasłem. Hasła haszowane Argon2id (parametry OWASP: m = 19 MiB, t = 2, p = 1).
- Sesja po stronie serwera (tabela `sessions` + cache w pamięci), identyfikator w ciasteczku `HttpOnly; Secure; SameSite=Lax`.
- WebSocket na tej samej domenie: przeglądarka wysyła ciasteczko przy nawiązywaniu połączenia, a serwer sprawdza sesję i nagłówek `Origin` (ochrona przed CSWSH).
- Żądania HTTP zmieniające stan wymagają nagłówka `X-Requested-With` (dodatkowa ochrona CSRF).
- Potwierdzenie e-maila zmienia konto z niepotwierdzonym adresem w pełne (R-KNT-02). Znaczenie terminu „gość” uściśla ADR-018.

**Alternatywy:** JWT w `localStorage` (odrzucone: ryzyko XSS, trudne unieważnianie); tylko logowanie zewnętrzne OAuth (zależność od dostawcy; możliwe jako dodatek później); passkeys / WebAuthn (dodatek w przyszłości).

**Konsekwencje.** Tabela sesji, rotacja identyfikatora po logowaniu, limity prób logowania (`12-bezpieczenstwo-i-prawo.md`). Aplikacja Android działa z innego pochodzenia, więc loguje się tokenem (ADR-015).

## ADR-011. Brak płatności za prawdziwe pieniądze na starcie

**Kontekst.** Decyzja właściciela projektu. Płatne losowe zestawy kart (loot boxy) są w części krajów regulowane. Płatności dokładają integrację, VAT, obsługę reklamacji i ochronę przed oszustwami.

**Decyzja.** Dukaty zdobywa się tylko w grze (R-EKO-01). Możliwa późniejsza faza opcjonalna: płatności za elementy niewpływające na rozgrywkę albo bez losowości.

**Konsekwencje.** Ekonomia musi być zrównoważona przez źródła i ujścia w grze. Wymaga monitoringu inflacji (`08-serwer.md`, rozdz. 6).

## ADR-012. Mechanika z oryginału, treść własna

**Kontekst.** Decyzja właściciela projektu. Nazwa „Cellmons”, nazwy postaci i kart, teksty, fabuła i grafiki są chronione prawem autorskim lub znakiem towarowym. Mechanika gry co do zasady nie jest chroniona.

**Decyzja.** Odtwarzamy zasady (z oznaczeniem w specyfikacji), a nazwy, teksty, fabułę, tytuły i grafiki tworzymy sami. W kodzie używamy neutralnych nazw. Dane referencyjne (`docs/dane/`) nie trafiają do produktu.

**Konsekwencje.** Prace projektowe nad treścią (`03-tresc-gry.md`, rozdz. 5.4). Przed publicznym startem konsultacja prawna (`12-bezpieczenstwo-i-prawo.md`, rozdz. 8).

## ADR-013. Wersja zasad: klasyczna 2004–2009

**Kontekst.** Decyzja właściciela projektu. Wersja 2010 dodała Eter, który zmieniał balans i wprowadzał natychmiastowe wygrane.

**Decyzja.** Implementujemy zasady klasyczne. Dane z 2010 (koszty, statystyki, zapisy walk) służą tylko jako referencja liczbowa.

**Konsekwencje.** Brak Eteru. Silnik ma jednak punkt rozszerzenia „zasoby pojedynku” (`06-silnik-gry.md`, rozdz. 3), więc dodatkowy zasób w przyszłości nie wymaga przebudowy.

## ADR-014. Aplikacja Android: Capacitor

**Kontekst.** Decyzja właściciela: gra ma mieć wersję na Androida, w Google Play i jako plik APK do pobrania. Priorytety: jeden kod klienta, wydajność, trening offline, szybkie aktualizacje.

**Decyzja.**
- Natywna powłoka Capacitor z tym samym klientem (SolidJS + silnik WASM) uruchomionym w Android System WebView.
- Pakiet web wbudowany w aplikację; nowe wersje interfejsu i treści jako podpisane paczki web, bez nowego APK.
- Funkcje natywne przez wtyczki.
- Dystrybucja: AAB w Google Play i podpisany APK ze strony, oba tym samym kluczem podpisu.

Szczegóły: `19-aplikacja-android.md`.

**Alternatywy:**

| Opcja | Za | Przeciw |
|---|---|---|
| TWA (Trusted Web Activity) | najmniej pracy, aktualizacje razem ze stroną | wymaga Chrome i sieci przy pierwszym uruchomieniu, mniej funkcji natywnych |
| Capacitor ładujący grę z serwera (zdalny URL) | aktualizacje jak na stronie | brak gry offline od startu, ryzyko odrzucenia w Google Play jako „opakowanie strony” |
| Tauri 2 (mobile) | silnik Rust natywnie w aplikacji | młodsze narzędzia mobilne, trudniejsze budowanie (NDK), mniej wtyczek; zysk wydajności niewielki, bo silnik w WASM już mieści się w budżecie |
| Natywnie (Kotlin, Jetpack Compose) | pełna natywność | drugi interfejs do napisania i utrzymania |
| Flutter / React Native | dojrzałe narzędzia mobilne | drugi interfejs i drugi język; silnik przez dodatkowe mosty |

**Konsekwencje.**
- (+) Jeden kod klienta. Gra offline od pierwszego uruchomienia, szybkie aktualizacje przez paczki web.
- (−) Różnice między wersjami WebView na starszych telefonach (minimalna wersja i komunikat o aktualizacji).
- (−) Dodatkowy potok budowania i podpisywania. Konieczność zabezpieczenia klucza podpisu.
- (−) Wymogi Google Play (konto organizacji, formularze, recenzje).
- (−) Drugi sposób logowania (ADR-015).

## ADR-015. Logowanie w aplikacji: token sesji + bilet WebSocket

**Kontekst.** Aplikacja Capacitor ładuje grę z pochodzenia `https://localhost`, a API ma inną domenę. Ciasteczko sesji z `SameSite=Lax` nie zostanie wysłane, a ciasteczka stron trzecich są coraz mocniej ograniczane.

**Decyzja.**
- Logowanie z `client = android` zwraca token sesji w treści odpowiedzi. Aplikacja przechowuje go zaszyfrowany kluczem z Android Keystore.
- HTTP z nagłówkiem `Authorization: Bearer`. WebSocket przez jednorazowy bilet ważny 30 s.
- Lista dozwolonych pochodzeń (CORS i `Origin`) obejmuje `https://localhost`.
- Logowanie w przeglądarce bez zmian (ADR-010).

**Alternatywy:**
- Ciasteczka `SameSite=None`: problemy z ciasteczkami stron trzecich i ryzyko CSRF.
- Natywne zapytania HTTP z ciasteczkami (`CapacitorHttp`): nie obejmują WebSocketa.
- Podawanie gry pod domeną API: koliduje z routingiem zapytań.
- Bezstanowe JWT: trudne unieważnianie.

**Konsekwencje.** Dwie ścieżki logowania do testowania. Nowe endpointy `ws-ticket` i `rotate`. Konfiguracja CORS. Tokeny widoczne na liście aktywnych sesji.

## ADR-016. Grafika wektorowa (SVG) tworzona przez asystenta AI

**Kontekst.** Decyzja właściciela: grafiki przygotowuje asystent AI (Claude), kolejno, element po elemencie. Asystent tworzy grafiki jako kod (SVG), a nie jako obrazy malowane.

**Decyzja.**
- Styl dopasowany do grafiki wektorowej: emblematy i sygile szkół, geometryczne, stylizowane portrety postaci, ramki kart według rzadkości, spójna paleta kolorów dla każdej szkoły.
- Pliki SVG optymalizowane przez SVGO. Wersje rastrowe (PNG) tylko tam, gdzie wymaga ich zewnętrzna usługa (ikona i grafiki Google Play, ikony PWA), renderowane automatycznie z SVG.
- Każdy element powstaje w cyklu: opis → 2–3 warianty → wybór i poprawki właściciela → wersja końcowa z wpisem w rejestrze pochodzenia (`12-bezpieczenstwo-i-prawo.md`, rozdz. 8).

**Konsekwencje.**
- (+) Bardzo małe pliki (zwykle kilka–kilkadziesiąt KB). Ostre na każdym ekranie, łatwe zmiany kolorów i motywów, brak kosztów zamówień.
- (−) Ograniczony realizm: styl musi być świadomie umowny. Spójność pilnuje przewodnik stylu (krok F3.6 planu).
- (−) Niepewna ochrona prawnoautorska grafik tworzonych głównie przez AI. Dokumentujemy wkład właściciela, a markę chronimy znakiem towarowym.

## ADR-017. Sesja treningu w WASM i wspólny pakiet treści

**Kontekst.** F5 potrzebuje trwałego stanu walki w workerze, wyliczania kosztów przez ten sam silnik i działania po odłączeniu sieci. Osobne ładowanie niezgodnych wersji klienta, treści i WASM prowadziłoby do rozbieżności kart i zasad.

**Decyzja.** `Training` w `engine-wasm` przechowuje sesję i przyjmuje binarne wiadomości przez `dispatch`. Protobuf `game.v1.TrainingRequest/TrainingReply` jest generowany dla Rust i TypeScript. Start weryfikuje wersję treści klienta z wersją `engine-content`. Budowanie tworzy jeden pakiet z JSON, tekstów Fluent, grafik i WASM. PWA instaluje cały ten pakiet, a Android dołącza go do APK. Ukryte dane i ruch AI pozostają w workerze; UI otrzymuje widok gracza i jawne zdarzenia. Profile botów są kompilowane z `bots.toml` w czasie budowania; test porównuje je z wynikiem walidowanego parsera. Ziarno treningu pochodzi z `crypto.getRandomValues`.

**Alternatywy.** Stan silnika w UI i serializowanie go po każdym ruchu zwiększa ryzyko ujawnienia danych i koszt transferów. Oddzielny parser zasad w TypeScript powielałby silnik. Parsowanie profili TOML w WASM dodawało ok. 68 KiB Brotli, mimo niezmiennej konfiguracji w danym wydaniu.

**Konsekwencje.** Jeden silnik i spójny, kontrolowany rozmiar pakietu offline. Zmiana treści wymaga przebudowania WASM oraz klienta. Aktualizacja nie przerywa aktywnego treningu, ale odświeżenie strony rozpoczyna nową sesję. Ten etap nie dodaje jeszcze zdalnego dostarczania treści, API podglądu online, konstruktora talii ani pełnego odtwarzania zapisanych powtórek; pozostają w swoich fazach planu.

## ADR-018. Gość wyłącznie lokalny, bez zapisu danych

**Kontekst.** Właściciel przy F6.2 zażądał wejścia jako gość bez logowania: dane i postęp mają istnieć wyłącznie na froncie i nigdzie nie być zapisywane. Wcześniej dokumentacja nazywała gościem konto zapisane w bazie, którego e-mail nie został potwierdzony.

**Decyzja.** „Graj jako gość” tworzy stan w pamięci klienta. Nie tworzy użytkownika, sesji ani tokenu na serwerze, nie wywołuje API i nie zapisuje danych w localStorage, sessionStorage, IndexedDB ani ciasteczkach. Ustawienia wybrane w tym trybie również pozostają w pamięci. Odświeżenie lub zamknięcie strony usuwa gościa i jego stan. Obecnie gość korzysta z treningu Rust/WASM offline. PWA może przechowywać statyczne pliki aplikacji do gry offline; nie zawierają danych gościa.

Konto z niepotwierdzonym e-mailem pozostaje trwałym kontem z ograniczeniami R-KNT-02. Potwierdzenie adresu usuwa te ograniczenia. Lokalny gość nie jest takim kontem i nie ma dostępu do operacji online. Ta decyzja uzupełnia ADR-010/015 w zakresie nazwy i zachowania gościa; sposoby uwierzytelniania zarejestrowanych kont pozostają w mocy.

**Alternatywy.** Anonimowy użytkownik w bazie lub token aplikacji wymagałyby zapisu na serwerze. Zapis klienta w pamięci trwałej pozwalałby wznowić grę, ale nie spełniałby wymagania właściciela.

**Konsekwencje.** Gość działa bez sieci i rejestracji, ale nie ma odzyskiwania ani automatycznego przenoszenia jego stanu na konto. Limity, ranking i nagrody online dotyczą kont uwierzytelnionych. Testy sprawdzają brak API i zapisu oraz utratę trybu po przeładowaniu strony.

## ADR-019. Wdrożenie po zakończeniu aktywności

**Kontekst.** Lobby jest lokalne dla procesu; dzienniki walk, turnieje i plany wypraw zawierają konkretną wersję silnika i treści. Przejęcie niezgodnej aktywności mogłoby zmienić wynik lub zablokować zasoby gracza.

**Decyzja.** Wspólna transakcyjna bariera blokuje nowe aktywności. Dotychczasowy slot kończy wszystkie aktywności; po ośmiu minutach wydanie zostaje odłożone. Kandydat uruchamia aktorów dopiero po opróżnieniu aktywności. Caddy przełącza jeden upstream. Stary proces jest zatrzymywany przed odblokowaniem bariery, aby stare WebSockety nie rozpoczęły walki tuż przed zamknięciem. Wycofanie po wznowieniu gry używa tej samej procedury. Bilety WS są jednorazowe we wspólnej bazie, a dzierżawy są filtrowane według wersji przed przejęciem.

**Konsekwencje.** Wdrożenie nie przerywa walk, ale może być odłożone przez długi turniej lub wyprawę. Nie ma przekazywania między różnymi silnikami ani równoważenia dwóch lokalnych lobby. Zmienia to wcześniejszy szkic infrastruktury, który zakładał automatyczne przekazanie po limicie czasu.

## ADR-020. Minimalne obrazy wykonawcze

**Kontekst.** Skan rzeczywiście zbudowanych obrazów ujawnił podatności bibliotek systemowych i nieużywanej obsługi SFTP.

**Decyzja.** Serwer Linux x86_64 jest statycznie linkowany z musl i działa jako UID 65532 w `distroless/static-debian13`. PostgreSQL 18 używa oficjalnego Alpine 3.24 z aktualizacją pakietów. pgBackRest 2.59.3 budujemy ze źródła zweryfikowanego SHA-256, bez opcjonalnego libssh2/SFTP; repozytoria używane przez wdrożenie to S3 i lokalne fixture. Budowanie wykonuje również test smoke producenta. Skan Grype nie ma wykluczeń podatności; kody błędów blokują wydanie.

**Konsekwencje.** Obrazy nadal wymagają regularnej przebudowy i skanu. Migracja istniejącej bazy między dystrybucjami wymaga osobnego odbioru danych, rozszerzeń i porządkowania znaków; nie jest zwykłym rolloutem aplikacji. Obraz odtwarzania musi odpowiadać źródłu kopii. Gotowy pipeline buduje x86_64; inna architektura wymaga osobnej kompilacji i testów.
