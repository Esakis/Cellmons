# Wdrożenie i obsługa

Repozytorium przygotowuje wydanie; nie zawiera rzeczywistej domeny, kont dostawców ani sekretów właściciela. Staging i produkcja wymagają oddzielnych hostów. Polecenia poniżej dotyczą przygotowanego hosta Ubuntu 24.04 LTS, nie komputera deweloperskiego.

## Pierwsze uruchomienie

1. Uruchom `ansible-playbook -i <inwentarz> deploy/ansible/host.yml`, podając `owner_ssh_public_key` i ścieżkę `owner_ssh_private_key_file`. Playbook sprawdza nowe połączenie SSH przed wyłączeniem poprzednich sposobów logowania. Operator z grupą Docker jest zaufanym administratorem hosta.
2. Umieść katalog `deploy/` w `/srv/spellgambit/deploy`. Utwórz `private/caddy`, `private/textfile`, `/srv/spellgambit/web/releases` i `/srv/spellgambit/web/assets`. Dostęp do `private` ogranicz do operatora i wymaganych UID kontenerów.
3. Z `server.example.env`, `slots.example.env`, `public.example.env` przygotuj `private/server.env`, `private/slots.env`, `private/public.env`. Wszystkie obrazy aplikacji/bazy wskazuj przez digest z artefaktu `images.json`. Hasła w URL bazy koduj jako komponent URI. W `private/migrator.env` zapisz osobny `DATABASE_URL` roli `game_migrator`; nie przekazuj go procesowi gry.
4. Utwórz losowe hasło administratora bazy w `private/postgres-password`. Przygotuj `private/pgbackrest.conf` według przykładu: rzeczywisty magazyn w UE, szyfrowanie, oddzielne uprawnienia do kopii i klucz zabezpieczony poza VPS. Konfiguracja jest montowana tylko do kontenera bazy; musi być czytelna przez jego użytkownika `postgres` (sprawdź UID obrazu).
5. Przygotuj domenę Cloudflare, tryb Full (strict), certyfikat origin w `private/caddy/origin.pem` oraz klucz `origin-key.pem`. Dodaj rzeczywiste sieci administratora do `private/admin-networks.json` jako tablicę CIDR. Zweryfikuj certyfikat i DNS przed wpuszczeniem testerów. Turnstile musi dopuszczać domenę przeglądarki oraz `localhost` używany przez Androida; serwer weryfikuje hostname i akcję tokenu.
6. W `private/active.slot` zapisz `blue`, a w `private/caddy/active.caddy` konfigurację z `active.example.caddy`. Uruchom samą bazę: `docker compose --env-file private/slots.env -f compose.production.yml up -d postgres`.
7. Jako administrator bazy wykonaj `postgres/roles.sql`, ustaw hasła ról przez `psql \password`, następnie uruchom usługę `migrate`. Wykonaj `postgres/privileges.sql` **jako `game_migrator`**. Migracje należą do migratora; proces gry nie otrzymuje uprawnień DDL.
8. Utwórz stanzę i pierwszą kopię: `docker compose --env-file private/slots.env -f compose.production.yml exec -u postgres postgres pgbackrest --stanza=game stanza-create`, następnie `check` i `--type=full backup`. Błędów archiwizacji WAL nie wolno pozostawić bez obsługi: zapełnią dysk.
9. Zainstaluj zatwierdzony pakiet klienta przez `install-web.py <pełny-SHA> < web.tgz`, ustaw `web/current` na `releases/<SHA>`. Uruchom jawnie `game-blue` i `caddy`; profile slotów zapobiegają przypadkowemu uruchomieniu obu wersji. Zatwierdzenie prawne musi obejmować konfigurację operatora z `legal/site.example.json`.
10. Uruchom jako root `update-firewall.py`. Lista Cloudflare pochodzi z oficjalnych endpointów HTTPS; atomowa aktualizacja dotyczy tylko tabeli `inet spellgambit`. Nie czyści reguł Dockera. Zapisana lista służy do startu bez zależności od sieci.
11. Zainstaluj jednostki systemd poza `spellgambit-restore.*`. Dodaj zależność `Requires=spellgambit-firewall.service` i `After=spellgambit-firewall.service` w drop-in `docker.service`, dopiero gdy zapisany `private/firewall.nft` został sprawdzony. Włącz timery maintenance, backup, telemetry, prune i firewall-refresh. Nie włączaj ogólnego `nftables.conf` czyszczącego reguły Dockera.

Pierwsze przejście z binarki sprzed F14 wymaga istniejącego trybu konserwacji i ręcznego bootstrapu schematu: ta binarka nie zna komendy `drain`. Kolejne wydania używają procedury poniżej.

## Kolejne wydania i wycofanie (R1–R2)

Workflow `Deploy` sprawdza zielone CI, Security, Android i Infrastructure dla tego samego SHA. Buduje obrazy, skanuje je Grype i publikuje w GHCR; archiwum klienta zawiera metadane wydania. Staging uruchamia się po zielonym CI. Produkcja wymaga ręcznego uruchomienia oraz GitHub Environment `production` z wymaganym zatwierdzeniem właściciela. Skonfiguruj te zabezpieczenia w GitHub; sam plik YAML nie ustanawia reviewerów.

Zmienne repozytorium: publiczny `LEGAL_CONFIG_JSON`. W każdym środowisku: `DEPLOY_HOST`, `DEPLOY_USER`, wcześniej zweryfikowany `DEPLOY_KNOWN_HOSTS` i sekret `DEPLOY_SSH_KEY`. Host musi mieć odczyt odpowiednich prywatnych obrazów GHCR. Klucz CI jest dostępem administracyjnym przez Docker; chroń go odpowiednio.

`release.sh <pełny-SHA> <server@sha256:...>`:

- wymaga poprawnej pełnej kopii młodszej niż 26 godzin i działającego archiwum WAL;
- blokuje nowe aktywności transakcyjną barierą, wykonuje zgodne wstecz migracje i sprawdza konfigurację kandydata;
- czeka do ośmiu minut na **wszystkie** pojedynki, turnieje, wyprawy i wymiany; po przekroczeniu czasu odkłada wydanie i pozostawia dotychczasowy serwer;
- uruchamia kandydata dopiero po opróżnieniu aktywności, porównuje SHA, silnik i treść serwera z klientem, przełącza Caddy, zatrzymuje stary slot przy nadal aktywnej barierze i odblokowuje grę;
- zachowuje poprzedni serwer w razie błędu. Przy nieudanym wycofaniu proxy pozostawia oba sloty oraz blokadę nowych aktywności do interwencji operatora.

Ta procedura nie przekazuje walk między różnymi silnikami. Rozpoczęte aktywności kończy dotychczasowa wersja. Odtwarzanie dzienników i harmonogram turniejów sprawdzają wersje przed przejęciem pracy. Lobby jest lokalne dla procesu, dlatego nie używaj równoważenia obu slotów jako klastra lobby. Długą wyprawę lub turniej trzeba zakończyć albo odłożyć wdrożenie; skrypt nie wymusza porażki gracza.

Wycofanie po przyjęciu nowych walk przeprowadza się **tą samą barierą**, z poprzednim SHA i digestem. Nie cofaj migracji przez kasowanie kolumn. Stare zasoby `/assets/` pozostają co najmniej siedem dni, a aktywne i dwa ostatnie wydania są chronione przed sprzątaniem.

Obraz PostgreSQL z `images.json` jest osobnym artefaktem do zaplanowanej aktualizacji bazy; rollout aplikacji nie restartuje bazy automatycznie.

## Monitoring i zadania (R5–R8)

`/metrics` działa na oddzielnym listenerze; porty 19091/19092 są publikowane wyłącznie na loopback. Publiczne Caddy odrzuca metryki, readiness i pliki wewnętrzne. `compose.monitoring.yml` uruchamia Prometheus, Alertmanager, node-exporter, Loki, Alloy i Grafanę wyłącznie na lokalnych adresach. Do Grafany używaj tunelu SSH, nie publicznego proxy bez dodatkowej ochrony.

Przed startem przygotuj `private/alertmanager.yml`, `private/owner-webhook-url`, `private/grafana-password`. Nie wstawiaj adresów odbiorców ani haseł do repozytorium. Usługi gry używają journald z tagiem nazwy kontenera; Alloy wybiera tylko `spellgambit-game-blue-*` i `spellgambit-game-green-*`. Playbook ustawia trwały journal z limitem 1 GiB i retencją 14 dni; Loki również przechowuje logi 14 dni. Pozostałe logi kontenerów mają rotację. Uwierzytelnienia, URL biletu i nagłówki zapytania nie trafiają do logów Caddy. Histogram rund obejmuje ręcznie przesłany ruch i jego trwały zapis; pełny pomiar sieci zapewnia `loadtest`.

`server maintenance` co dziesięć minut usuwa wygasłe dane uwierzytelnienia, wykonuje istniejącą retencję czatu/poczty, realizuje usunięcia kont po karencji, odświeża sygnały do **ręcznego** przeglądu i uzgadnia księgę. Zadanie ma wspólną blokadę w bazie, limit czasu i stan ostatniego powodzenia. Rozbieżność księgi daje niezerowy kod zakończenia i alarm; nie ma automatycznej korekty pieniędzy ani automatycznych kar.

Tygodniowy `server telemetry-report` tworzy agregaty rejestracji, aktywności kont, PD, Dukatów oraz wyników postaci i kart, rozdzielone według wersji i trybu. Grupy poniżej 20 gier są pomijane. Analiza powtórek ma jawny limit 20 000 najnowszych walk i flagę obcięcia. Nie stanowi analizy przyczynowej balansu; nie zbiera aktywności gości.

Alarmy dotyczą obu niedostępnych slotów, opóźnienia rund, błędów HTTP, zasobów hosta, puli bazy, konserwacji, kopii i archiwizacji WAL. Osobny zewnętrzny uptime monitor i rzeczywiste doręczenie testowego alertu właścicielowi wymagają konfiguracji jego usług. Monitoring na tym samym VPS nie wykryje skutecznie własnej utraty zasilania.

## Kopie, awaria i prywatność (R3–R4, R9–R13)

`restore-drill.mjs` odtwarza pełny schemat i syntetyczną księgę z szyfrowanej kopii lokalnej, sprawdzając granicę PITR. Do rzeczywistego S3 przygotowano `restore-s3.sh` oraz miesięczny timer na **oddzielnej maszynie odtwarzania** (`/srv/spellgambit-restore`, osobna konfiguracja z uprawnieniami odczytu repozytorium). Skrypt tworzy własny nowy wolumin, nie montuje produkcyjnego PGDATA, sprawdza księgę i limit jednej godziny. Raportów oraz metryk powodzenia nie należy zastępować samym statusem zakończenia `restore`.

Przy rzeczywistej awarii: wstrzymaj ruch → zabezpiecz stary wolumin → odtwórz na nowym → sprawdź migracje, dane i księgę → uzgodnij najnowsze operacje administracyjne i wnioski prywatności → unieważnij sesje i tokeny pocztowe → wykonaj zaległe usunięcia kont → testy dymne → dopiero wtedy otwórz ruch. Zachowaj klucze szyfrowania kopii oraz historyczne poświadczenia potrzebne do odtworzenia poza VPS. Nie podłączaj starego i odtworzonego woluminu do jednego procesu PostgreSQL.

Publiczne dokumenty opisują pseudonimizację zachowanej księgi i historii walk; nie obiecują pełnej anonimizacji tych UUID. Dodatkowe okresy przechowywania audytu wymagają decyzji właściciela i odbioru prawnego. Wyciek klucza Androida i błędna paczka web mają procedury w `docs/36-android-aktualizacje-i-wydania.md`.

## Strona APK

Chronione wydanie Android zawiera `downloads/` przygotowane przez `scripts/prepare-download.mjs`: APK z nazwą zależną od SHA, `index.html`, CSS, `release.json` i `SHA256SUMS`. Przed publikacją zweryfikuj podpis APK narzędziem `apksigner`, tożsamość aplikacji, SHA i zgodność z działającym serwerem. Umieść katalog pod `/srv/spellgambit/web/downloads` (Caddy obsługuje `/downloads/`). Najpierw skopiuj wersjonowany APK, potem atomowo podmień stronę i metadane. Nie nadpisuj istniejącego APK innymi bajtami. Workflow nie publikuje automatycznie do Play ani na cudzy CDN.

Paczka aktualizacji `web-bundle/` używa oddzielnego klucza Ed25519. ZIP trafia przed manifestem; manifest dopiero po aktywacji zgodnego serwera. Numer manifestu zawsze rośnie, także przy naprawie. Testowe artefakty i przykładowe dane operatora nie są materiałem do publicznej publikacji.

Obrazy docelowe: serwer statyczny musl na x86_64 w distroless/static oraz PostgreSQL 18/Alpine 3.24 z pgBackRest 2.59.3 z przypiętego źródła. Obsługa SFTP jest wyłączona; używamy S3, a lokalne próby korzystają z repozytorium na osobnym woluminie. Obrazu odtwarzania nie dobieraj przypadkowo: musi odpowiadać środowisku źródłowej kopii. Zmianę dystrybucji istniejącej bazy, rozszerzeń i kolacji wykonuje się jako osobną migrację z odbiorem, nigdy przez automatyczne podmienienie obrazu produkcji.
