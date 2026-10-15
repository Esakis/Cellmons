# 08. Serwer gry

Serwer to jedna binarka Rust (crate `server`): HTTP i WebSocket (Axum), cała logika domenowa, aktory stanu współbieżnego i dostęp do PostgreSQL (ADR-002, ADR-005). Protokół opisuje `07-protokol-sieciowy.md`, a schemat bazy `09-baza-danych.md`.

**Stan F6.2 (2026-10-15).** Działają konfiguracja, telemetria, `/healthz`, `/readyz`, `server migrate` i API kont: rejestracja, logowanie web/aplikacji, sesje, tokeny e-mail, reset, rotacja i bilety WebSocket. Schemat i zapytania sprawdzają testy PostgreSQL oraz SQLx. Lokalny gość działa wyłącznie na froncie, bez konta w bazie (ADR-018). `/readyz` zwraca 503 także przy braku migracji. Pozostałe moduły poniżej opisują docelowy serwer; następny krok to gateway F6.3. Szczegóły: [raport F6.2](23-konta-i-gosc-testy.md).

## 1. Proces i runtime

- Tokio w trybie wielowątkowym: liczba wątków = liczba rdzeni.
- Obliczenia AI (poziom L3) nie blokują runtime'u: trafiają do osobnej puli wątków (`rayon`) o ograniczonym rozmiarze, z limitem czasu na decyzję.
- Alokator `mimalloc` (szybszy przy wielu małych alokacjach i wielu wątkach).
- Profil `release`: `lto = "fat"`, `codegen-units = 1`, `panic = "unwind"`. Panika w jednym aktorze nie zabija procesu: zadanie kończy się błędem, a nadzorca go obsługuje (rozdz. 4.3).
- Konfiguracja: zmienne środowiskowe (rozdz. 13). Treść gry jest wkompilowana (`engine-content`), więc serwer i klient zawsze mają tę samą wersję treści dla danego wydania.

## 2. Moduły

| Moduł | Odpowiedzialność | Stan w pamięci | Dane w bazie |
|---|---|---|---|
| `gateway` | HTTP (Axum), WebSocket, dekodowanie Protobuf, routing do modułów, limity | połączenia | — |
| `auth` | rejestracja, logowanie, sesje przeglądarki i tokeny aplikacji, bilety WebSocket (ADR-015), weryfikacja e-mail, reset hasła | cache sesji (LRU), pamięć użytych biletów (30 s) | `users`, `sessions`, `email_tokens` |
| `presence` | kto jest online, w którym pokoju, status AFK, filtr wyzwań | rejestr obecności | — |
| `rooms` | aktory pokojów, czat, lista ostatnich pojedynków | aktory pokojów | `chat_log` (moderacja) |
| `social` | znajomi, ignorowani, poczta | — | `friends`, `ignores`, `mail` |
| `challenges` | wyzwania, wygasanie, blokada „jedno naraz” (R-LOB-06) | rejestr wyzwań | — |
| `duels` | aktory pojedynków, limity czasu, dziennik rund, odtwarzanie po restarcie | aktory pojedynków | `duel_journal`, `duel_journal_rounds`, `matches` |
| `progression` | PD, poziomy, PR, reinkarnacje, cienie, odblokowania postaci | — | `avatars`, `unlocked_templates` |
| `rating` | ranking, tytuły, ranking klanów | cache rankingu | `ratings`, `rating_history` |
| `collection` | kolekcja kart, talie, zapisane zestawy | — | `collection`, `decks`, `deck_presets` |
| `economy` | portfel, księga, sklep, zestawy kart, wysypisko, limity dzienne | — | `wallets`, `ledger_entries`, `idempotency_keys`, `boosters`, `junkyard`, `daily_counters` |
| `trade` | aktory wymiany, atomowe rozliczenie | aktory wymiany | `trades`, `trade_items` |
| `tournaments` | aktory turniejów, drabinki, harmonogram, nagrody | aktory turniejów | `tournaments`, `tournament_entries`, `tournament_matches` |
| `expeditions` | przebieg wypraw, generowanie etapów, nagrody | aktor wyprawy na gracza | `expeditions` |
| `bots` | boty sali treningowej, AI dla wypraw | menedżer botów | — |
| `clans` | klany, role, wnioski, ranking klanów | — | `clans`, `clan_members`, `clan_applications` |
| `admin` | panel, blokady, ogłoszenia, turnieje systemowe, tryb konserwacji | — | `bans`, `reports`, `audit_log` |
| `antiabuse` | wykrywanie nadużyć (R-FAI-01), anomalie ekonomii | liczniki | `abuse_signals` |
| `jobs` | zadania okresowe (rozdz. 11) | harmonogram | — |
| `telemetry` | metryki, ślady, logi | — | — |

Moduły udostępniają **usługi** (struktury z metodami asynchronicznymi) i **uchwyty aktorów** (`Handle`, które wysyłają wiadomości). Nie sięgają do wnętrza innych modułów.

## 3. Struktura kodu

```
crates/server/src/
  main.rs            # start, konfiguracja, budowa routera, zamykanie
  config.rs
  gateway/           # http.rs, ws.rs, codec.rs, rate_limit.rs, router.rs
  auth/ presence/ rooms/ social/ challenges/ duels/ progression/ rating/
  collection/ economy/ trade/ tournaments/ expeditions/ bots/ clans/ admin/ antiabuse/
  db/                # pula połączeń, migracje, wspólne typy
  jobs/
  telemetry/
```

Każdy moduł domenowy ma podział: `service.rs` (logika i transakcje), `actor.rs` (jeśli ma stan współbieżny), `repo.rs` (zapytania `sqlx`), `messages.rs` (mapowanie z i do Protobuf), `tests.rs`.

## 4. Model współbieżności

### 4.1. Aktory

| Aktor | Ile | Skrzynka | Czas życia |
|---|---|---|---|
| Połączenie (zadanie czytające + zadanie piszące) | 1 na połączenie | wyjściowa: ograniczona, 256 wiadomości | do zamknięcia połączenia |
| Pokój | 1 na pokój (stałe + 1 na klan) | 1024 | stały, pokoje klanów tworzone na żądanie |
| Pojedynek | 1 na trwający pojedynek | 64 | od `DuelStart` do zapisania wyniku |
| Wymiana | 1 na trwającą wymianę | 64 | do zakończenia lub anulowania |
| Turniej | 1 na aktywny turniej | 256 | od utworzenia do wypłaty nagród |
| Wyprawa | 1 na gracza w wyprawie | 32 | do końca wyprawy |
| Menedżer botów | 1 | 256 | stały |

### 4.2. Zasady

- **Brak współdzielonego, modyfikowalnego stanu między aktorami.** Rejestry (obecność, wyzwania, aktywne pojedynki) używają sharded map (`DashMap`), ale tylko do wyszukiwania uchwytów. Stan domenowy zawsze należy do jednego aktora.
- **Kanały ograniczone.** Gdy kolejka wyjściowa połączenia się zapełni (wolny klient), połączenie jest zamykane, a gracz może wznowić sesję (`07-protokol-sieciowy.md`, rozdz. 2). Chroni to serwer przed puchnięciem pamięci.
- **Rozsyłanie bez kopiowania:** wiadomość do wielu odbiorców (czat, zmiany w pokoju) jest kodowana raz do `Bytes`, a odbiorcy dostają tani klon wskaźnika.
- **Bez blokowania runtime'u:** żadnych synchronicznych operacji I/O ani długich obliczeń w zadaniach Tokio. AI L3 idzie do puli `rayon`.
- **Kolejność blokad w bazie:** operacje dotyczące dwóch graczy (wymiana, wynik pojedynku) blokują wiersze w kolejności rosnącego `user_id`, co zapobiega zakleszczeniom.

### 4.3. Nadzór i awarie

- Każdy aktor uruchamia się przez nadzorcę, który obserwuje jego `JoinHandle`.
- Panika aktora pojedynku: nadzorca loguje błąd (z `duel_id` i ziarnem) i odtwarza pojedynek z dziennika rund (rozdz. 5.3). Jeśli odtworzenie się nie uda, kończy pojedynek remisem technicznym bez zmian rankingu i zgłasza alert.
- Panika aktora wymiany: wymiana jest anulowana, nic nie zostało przeniesione, bo rozliczenie jest jedną transakcją.
- Panika aktora turnieju: turniej odtwarza się ze stanu w bazie (drabinka i wyniki faz są zapisywane na bieżąco).

## 5. Pojedynki

### 5.1. Cykl życia

1. Moduł `challenges` (albo turniej, wyprawa, sala treningowa) zleca utworzenie pojedynku: snapshoty postaci z bazy (R-POJ-01), talie, ziarno z `getrandom`.
2. Aktor pojedynku tworzy stan (`engine::new_duel`), zapisuje nagłówek w `duel_journal` (snapshoty, ziarno, wersje) i wysyła `DuelStart` (z `PlayerView` każdego gracza).
3. Pętla rund: czekanie na ruchy z limitami czasu (`tokio::time::sleep_until`), walidacja (`engine::validate_move`), po dwóch ruchach `engine::resolve_round`, zapis ruchów do dziennika (asynchronicznie, bez czekania na zapis przed wysłaniem wyniku), wysłanie `DuelRoundResult` z przefiltrowanymi zdarzeniami.
4. Koniec: transakcja wyniku (rozdz. 5.2), `DuelEnd`, zamknięcie aktora, usunięcie wpisu z dziennika aktywnych.

### 5.2. Transakcja wyniku

Jedna transakcja w bazie:
1. Zablokowanie wierszy `ratings` obu graczy (`SELECT … FOR UPDATE`, kolejność według `user_id`).
2. Obliczenie zmiany rankingu (R-RNK-02) i PD (R-PD-02) na wartościach z blokady.
3. Zapis `matches` (wynik, powód, czas, delta rankingu, PD, powtórka).
4. Aktualizacja `ratings`, `rating_history`, `avatars` (PD, poziom, PR), statystyk gracza (gry, W/R/P, rozłączenia).
5. Nagrody: Dukaty (wpis w księdze, R-EKO-01), szansa na postać (R-NAG-01): losowanie po stronie serwera, poza silnikiem.
6. Zatwierdzenie transakcji, potem powiadomienia (`DuelEnd`, `ProfileUpdate`, `AvatarUpdate`).

Jeśli transakcja się nie powiedzie (np. utrata połączenia z bazą), aktor ponawia ją z wykładniczym opóźnieniem. Wynik jest idempotentny: `matches.id` = `duel_id`, więc powtórzony zapis nic nie zmienia.

### 5.3. Odtwarzanie po restarcie

- `duel_journal` i `duel_journal_rounds` przechowują dla każdego aktywnego pojedynku nagłówek (setup) i ruchy każdej rozstrzygniętej rundy.
- Przy starcie serwer wczytuje niezakończone pojedynki, odtwarza stan przez `engine::replay`, tworzy aktory i ustawia nowe limity czasu (gracze dostają czas od nowa, R-KON-05).
- Gracze po ponownym połączeniu dostają `DuelSnapshot` i grają dalej.
- To samo działa przy wdrożeniu nowej wersji (rozdz. 9): pojedynek, który nie skończył się przed wygaszeniem starej instancji, dostaje status `handover`, a stara instancja wysyła `NOTIFY duel_handover`. Nowa instancja (nasłuchuje przez `LISTEN`) przejmuje go, ustawiając siebie w `owner_instance` warunkowym `UPDATE`. Dzięki temu pojedynku nie przejmą dwie instancje.

### 5.4. Pojedynki z AI

Pojedynek z botem (trening online, wyprawa) to ten sam aktor. Ruch strony AI liczy `engine-ai` w puli `rayon` z limitem czasu zależnym od poziomu (L0–L2: < 5 ms, L3: ≤ 50 ms). Bot wysyła ruch od razu po rozpoczęciu rundy (R-TRE-01).

## 6. Ekonomia

Wszystkie operacje ekonomiczne to transakcje w bazie z więzami integralności (`09-baza-danych.md`, rozdz. 4):

| Operacja | Kroki w transakcji | Ochrona |
|---|---|---|
| Zakup zestawu | sprawdzenie i obciążenie portfela, wpis w księdze, utworzenie zestawu | `idempotency_key` unikalny; `CHECK (balance >= 0)` |
| Otwarcie zestawu | losowanie zawartości (RNG serwera), dopisanie kart do kolekcji, oznaczenie zestawu jako otwartego | status `unopened → opened` z warunkiem w `UPDATE` |
| Wysypisko | blokada wierszy kolekcji, odjęcie kart, dopisanie punktów, przyznanie nagrody przy progu (R-WYS) | `CHECK (qty >= 0)`; idempotencja |
| Wymiana | blokada portfeli i kolekcji obu stron (kolejność `user_id`), sprawdzenie stanów, przeniesienie kart i Dukatów, księga, zapis wymiany | akceptacja przypisana do `offer_version` (R-WYM-03) |
| Nagroda (walka, wyprawa, turniej) | wpis w księdze + zmiana salda | limity dzienne w `daily_counters` |

**Księga transakcji:** każda zmiana salda Dukatów to wpisy w `ledger_entries` (konto, kwota, powód, odnośnik), sumujące się w obrębie operacji do zera (`09-baza-danych.md`, rozdz. 3.5). Zadanie okresowe porównuje saldo portfela z sumą księgi; rozbieżność to alert krytyczny.

**Monitoring ekonomii (ADR-011):** metryki dziennej podaży (źródła) i popytu (ujścia) Dukatów, mediana i 95. percentyl sald, liczba otwieranych zestawów. Gdy podaż przez tydzień przekracza popyt o więcej niż 20%, to sygnał do korekty parametrów `economy.*`.

## 7. Turnieje

Aktor turnieju (R-TUR-01…09):
1. **Zapisy:** walidacja postaci (poziom, reinkarnacja, klan), pobranie wpisowego do puli (księga, konto techniczne turnieju), lista uczestników w bazie.
2. **Start:** po zebraniu kompletu. Drabinka losowana ziarnem turnieju (zapisanym, czyli odtwarzalna); wolne losy według Z-TUR.
3. **Fazy:** dla każdej pary seria walk do N. Pierwsza walka startuje 20 s po starcie turnieju, każda następna 15 s po powrocie obu graczy (R-TUR-06). Gracz nieobecny przegrywa walkowerem. Punkty 2/1/0, zakończenie fazy, gdy przeciwnik nie może dogonić, remis rozstrzygany według `tournament.tiebreak`.
4. **Turniej klanowy:** suma punktów klanu decyduje o awansie całego klanu (R-TUR-08).
5. **Nagrody:** wypłata z puli w jednej transakcji. Dla turniejów systemowych nagroda pochodzi z konta systemowego.
6. **Trwałość:** stan drabinki i wyniki faz są zapisywane po każdej walce, co pozwala odtworzyć turniej po restarcie (rozdz. 4.3).

Turnieje systemowe (o postać, kartę, zestaw, Dukaty) tworzy zadanie okresowe według harmonogramu w konfiguracji oraz administrator z panelu.

## 8. Wyprawy

Aktor wyprawy (jeden na gracza, R-WYP-01…06):
- Generuje etapy deterministycznie z ziarna wyprawy: grupy 3 przeciwników z pul treści (`expeditions.toml`) z poziomami względem postaci gracza. Kolor przeciwnika wynika z różnicy poziomów.
- Każda walka to zwykły aktor pojedynku ze stroną AI.
- Liczy zebrane PD; przy wycofaniu wypłaca część (R-WYP-04). Po pokonaniu bossa losuje nagrodę (R-WYP-05) i wypłaca Dukaty za ukończenie (R-EKO-01).
- Stan wyprawy jest zapisywany po każdej walce, więc przerwana wyprawa (rozłączenie, restart) jest wznawiana od bieżącego etapu.

## 9. Start, zamykanie, wygaszanie (drain)

**Migracje przed startem:** osobne polecenie `server migrate` (lokalnie wywołuje je `run.bat`, w produkcji krok wdrożenia z rolą DDL).

**Docelowy start:** wczytanie konfiguracji → przygotowanie puli bazy i sprawdzenie schematu → odtworzenie pojedynków, turniejów i wypraw → start menedżera botów → otwarcie portu → `/readyz` zwraca 200. W obecnym F6.1 serwer otwiera port także przy niedostępnej bazie, a stan połączenia i schematu pokazuje `/readyz`.

**Wygaszanie** (sygnał `SIGTERM` albo `POST /admin/drain`), używane przy każdym wdrożeniu:
1. `/readyz` zwraca 503, więc Caddy kieruje nowe połączenia do nowej instancji.
2. Ogłoszenie w pokojach: „konserwacja za X min” (`ServerNotice`).
3. Blokada nowych wyzwań, turniejów, wypraw i wymian (`MAINTENANCE`).
4. Czekanie, aż trwające pojedynki i wymiany się zakończą, maksymalnie `DRAIN_TIMEOUT` (domyślnie 8 min; pojedynek trwa najwyżej ok. 5 × 90 s).
5. Po upływie limitu: zapis dziennika pozostałych pojedynków (przejmie je nowa instancja), zamknięcie połączeń z kodem „wznów sesję”.
6. Zamknięcie puli bazy i wyjście.

Szczegóły przełączania instancji: `15-infrastruktura-i-wdrozenie.md`, rozdz. 6.

## 10. Boty sali treningowej

- Menedżer utrzymuje w sali treningowej od 6 do 12 botów (nazwy z prefiksem „AI”) na różnych poziomach i z różnymi postaciami (R-TRE-01).
- Co 5 minut część botów zmienia postać (R-TRE-02, Z-BOT).
- Boty akceptują wyzwania natychmiast. Każdy bot może prowadzić wiele pojedynków jednocześnie (to tylko logika, nie gracz).
- Walki z botami nie zmieniają PD, rankingu ani statystyk.

## 11. Zadania okresowe

| Zadanie | Częstotliwość | Opis |
|---|---|---|
| Czyszczenie sesji | co godzinę | usunięcie wygasłych sesji i tokenów e-mail |
| Reset liczników dziennych | o północy (Europe/Warsaw) | limity Dukatów za wygrane, pierwsza wygrana dnia |
| Uzgodnienie księgi | co godzinę | saldo portfeli = suma księgi; rozbieżność to alert |
| Ranking | co minutę | odświeżenie cache rankingu i tytułów |
| Turnieje systemowe | według harmonogramu | tworzenie turniejów o nagrody |
| Partycje | raz dziennie | tworzenie partycji `matches` na kolejny miesiąc |
| Retencja | raz dziennie | usunięcie dziennika czatu > 30 dni, zakończonych wpisów `duel_journal` > 7 dni, kont oczekujących na usunięcie po karencji |
| Wykrywanie nadużyć | co 15 min | sygnały „podkładania się” i wielu kont (`12-bezpieczenstwo-i-prawo.md`, rozdz. 4) |
| Kolejka e-maili | ciągle | wysyłka z ponawianiem |

## 12. Panel administracyjny

Osobny zestaw endpointów `/api/admin/*` i osobny pakiet klienta (nie trafia do pakietu gracza):
- wyszukiwanie graczy, profil, historia walk, księga;
- blokady (czasowe i stałe), wyciszenie czatu, reset PD postaci (kara za „podkładanie się”);
- zgłoszenia od graczy i dziennik czatu;
- turnieje systemowe i ogłoszenia;
- tryb konserwacji i wygaszanie;
- sygnały nadużyć i anomalie ekonomii;
- nadawanie ról (admin, moderator, mentor).

Każda operacja administracyjna trafia do `audit_log` (kto, co, kiedy, przed i po).

## 13. Konfiguracja

| Zmienna | Opis | Domyślnie |
|---|---|---|
| `DATABASE_URL` | połączenie z PostgreSQL | — (wymagana) |
| `MIGRATION_DATABASE_URL` | połączenie roli DDL, używane wyłącznie przez `server migrate` | `DATABASE_URL` |
| `DB_POOL_SIZE` | rozmiar puli | 2 × liczba rdzeni |
| `LISTEN_ADDR` | adres i port | `0.0.0.0:8080` |
| `PUBLIC_ORIGIN` | dozwolony nagłówek `Origin` | — (wymagana) |
| `APP_ORIGINS` | pochodzenia aplikacji mobilnej (CORS, `Origin` z biletem) | `https://localhost` |
| `WS_TICKET_KEY` | klucz HMAC biletów WebSocket (sekret) | — (wymagana) |
| `APP_MIN_NATIVE_VERSION` | najstarsza akceptowana wersja powłoki Android | `1.0.0` |
| `SESSION_TTL_DAYS` | ważność sesji | 30 |
| `SMTP_*` / `EMAIL_API_KEY` | dostawca e-mail | — |
| `MAIL_MODE` / `MAIL_OUTBOX` | F6.2: `file` tylko na loopback; lokalne wiadomości w plikach JSON / katalog wyjściowy | `file` / `target/mail` na loopback; poza nim `smtp` |
| `SMTP_HOST`, `SMTP_PORT`, `SMTP_USER`, `SMTP_PASSWORD`, `SMTP_FROM` | F6.2: serwer SMTP z obowiązkowym STARTTLS, konto, hasło i nadawca | port 587; pozostałe wymagane dla SMTP |
| `TURNSTILE_SITE_KEY`, `TURNSTILE_SECRET` | F6.2: publiczny/prywatny klucz antyspamu; wymagane poza loopback | wyłączone na loopback |
| `TRUSTED_PROXY_IPS` | F6.2: adresy IP pośredników, od których można przyjąć `X-Forwarded-For`, rozdzielone przecinkami | puste; nagłówek jest ignorowany |
| `DRAIN_TIMEOUT_S` | maks. czas wygaszania | 480 |
| `FEATURES` | flagi funkcji, np. `trading,tournaments` | wszystkie włączone |
| `OTEL_EXPORTER_OTLP_ENDPOINT` | eksport śladów | puste (wyłączone) |
| `LOG_LEVEL` | poziom logów (`error`, `warn`, `info`, `debug`, `trace`, `off`), opcjonalnie z poziomami modułów, np. `info,sqlx=warn`; nieznane słowo albo spacja po przecinku zatrzymują start z błędem | `info` |
| `LOG_FORMAT` | format logów: `json` (produkcja) albo `text` (czytelny przy pracy lokalnej) | `json` |
| `ADMIN_IP_ALLOWLIST` | adresy z dostępem do panelu | puste (brak dostępu) |

Sekrety (hasło bazy, klucze) tylko w zmiennych środowiskowych lub plikach Docker secrets, nigdy w repozytorium.

## Implementacja lobby (F6.3–F7.1)

`online/ws.rs` obsługuje autoryzację, Origin, limity, Protobuf, puls i zamykanie. `online/hub.rs` jest aktorem sesji, obecności, relacji i wyzwań; `online/room.rs` utrzymuje osobne, ograniczone kolejki pokojów, historię w RAM i rozsyłanie. `online/repo.rs` zawiera sprawdzane makrami SQLx zapytania i przygotowanie postaci/talii pod blokadami w transakcji. Obsługa polecenia ma limit 5 s. Wyjście/zmiana pokoju i rozłączenie anulują wyzwania. Odbiór rezerwacji pojedynku jest jednokrotny, a zakończenie zwalnia blokadę.

`Hub::is_locked` obejmuje wysłane wyzwanie i rezerwację walki. Mutacje F8 `/api/game` są szeregowane z wyzwaniami przez Hub i sprawdzają trwałą blokadę pod blokadą wiersza użytkownika. Wyzwania są w RAM jednej instancji. Zaakceptowana walka ma dodatkowo blokady `duel_locks` w bazie. Aktor, dziennik i dzierżawy właściciela są w `duel/`; odtwarzanie po zabiciu procesu sprawdza test chaosu. Mutacje F8 muszą pod blokadą wiersza użytkownika sprawdzać trwałe blokady, a zmiana aktywnej postaci anuluje oczekujące wyzwania. Godzinne sprzątanie usuwa log czatu starszy niż 30 dni i listy usunięte przez obie strony. Domyślne limity: `WS_CONNECTIONS_PER_IP=20`, `WS_MAX_CONNECTIONS=5000`.

Szczegóły F7.2–F7.5, limity, rozliczenie raz i ograniczenia wdrażania nowych wersji: [raport F7](25-pojedynki-online-testy.md).

## Ekonomia i wymiana F9

`economy.rs` obsługuje przepływy, nagrody arenowe, portfel i godzinne uzgodnienie; `store.rs` zakup, otwarcie i wysypisko; `trade.rs` ograniczone aktory wymiany z dzierżawą. Wspólne `activity.rs` chroni zasoby pomiędzy domenami pod blokadami użytkowników. Hub szereguje zaproszenia z pokojami i wyzwaniami; aktor wymiany wykonuje ofertę i obie akceptacje na trwałym dzienniku. Rozłączenie/wyjście z pokoju anuluje ofertę. [Raport F9](27-ekonomia-sklep-wymiana-testy.md).
