# F14 — przygotowanie produkcji i odtwarzania

Przygotowano kod, konfiguracje i lokalne dowody działania. Nie utworzono ani nie opublikowano rzeczywistej produkcji, DNS, magazynu S3 czy wpisu Google Play. M6 pozostaje otwarty do wykonania zewnętrznych odbiorów.

## Wydanie bez przerywania aktywności

`deploy/README.md` jest instrukcją uruchomienia. Serwer ma obraz bez roota i powłoki, rozdzielone role bazy, osobny prywatny listener metryk oraz identyfikator wydania, silnika i treści w healthchecku. Caddy, zapora, Ansible i Compose przygotowują host Ubuntu 24.04; rzeczywiste klucze i adresy znajdują się poza repozytorium.

`release.sh` blokuje równoczesne wdrożenia i nie przejmuje istniejącej blokady operatora. Sprawdza kopię, blokuje transakcyjnie nowe aktywności, wykonuje migracje expand, sprawdza konfigurację kandydata i czeka na zakończenie wszystkich pojedynków, wymian, turniejów oraz wypraw. Po ośmiu minutach odkłada wydanie. Nie poddaje graczy ani nie przekazuje walk do innego silnika.

Kandydat startuje po opróżnieniu aktywności. Proxy używa jednego aktywnego slotu; lokalne lobby nie udaje klastra. Wydanie porównuje SHA, silnik i treść klienta z serwerem. Instalator odrzuca niebezpieczne archiwa i kolizje zasobów; poprzednie fragmenty klienta są przechowywane co najmniej siedem dni. Wycofanie po rozpoczęciu nowych walk wymaga takiej samej bariery.

Migracje 023–026 dodają wersje turniejów/wypraw, wspólne jednorazowe bilety WS, barierę i stan zadań. Historyczne domyślne wersje pozostają dla starszego piszącego procesu; obecne inserty zawsze podają wersję jawnie. Harmonogram i odtwarzanie sprawdzają zgodność **przed** przejęciem pracy. Pierwsza aktualizacja binarki sprzed F14 wymaga ręcznego okna konserwacji.

Workflowy `Deploy`, `Infrastructure`, `Security` i `Android` są przypięte do narzędzi i ograniczonych uprawnień. Staging oczekuje zielonych kontroli tego samego SHA; produkcja wymaga ręcznego uruchomienia i chronionego GitHub Environment. Reviewerów i sekrety trzeba skonfigurować w prawdziwym repozytorium — YAML nie ustanawia tych zabezpieczeń samodzielnie.

## Kopie i monitoring

Obraz PostgreSQL 18 zawiera pgBackRest, szyfrowane repozytorium, ciągłą archiwizację WAL i codzienny backup. Próba `scripts/restore-drill.mjs` tworzy własne świeże woluminy, wykonuje pełną kopię, zapisuje dwa znaczniki i odtwarza punkt między nimi. Odtworzona księga musi się uzgodnić; wspólne zapytanie produkcyjne sprawdza migracje, portfele, depozyty i transakcje. Lokalne szyfrowane repozytorium nie jest dowodem RTO rzeczywistego S3.

`restore-s3.sh` i miesięczny timer służą osobnej maszynie odtwarzania z uprawnieniami odczytu repozytorium. Nie montują produkcyjnego PGDATA. Runbook zawiera ponowną realizację wniosków usuwania kont po przywróceniu starej kopii.

Prometheus, Alertmanager, node-exporter, Grafana, Loki i Alloy używają prywatnych adresów. Alarmy obejmują niedostępność, błędy, opóźnienia, zasoby, pulę bazy, konserwację, kopie i nieudaną archiwizację WAL. Logi mają ograniczoną retencję; Alloy wybiera tylko kontenery gry. Żądania z biletami i nagłówkami uwierzytelnienia nie trafiają do logów Caddy. Uptime z zewnętrznej sieci, odbiorca alarmów i ewentualna usługa śledzenia błędów wymagają kont właściciela.

## Lokalne dowody

- Test PostgreSQL z prawdziwą walką WS: aktywny pojedynek trwa podczas blokady nowych aktywności, kończy się, następny proces odczytuje ten sam wynik i powtórkę.
- Test bariery: rozpoczęcie blokady czeka na transakcję rezerwującą aktywność; jej wyłączenie nie wyłącza osobnego trybu konserwacji.
- Dwie osobne instancje uwierzytelnienia konkurują o bilet WS z jednej bazy: tylko jedna zużywa nonce. Rotacja tokenu usuwa bilety bez naruszenia klucza obcego.
- Rzeczywisty produkcyjny Caddy, własny CA i ścisły HTTPS: pliki prywatne zwracają 404, stary fragment JS pozostaje dostępny, klient nie może podmienić IP przez XFF, log błędu nie zawiera syntetycznego tokenu/cookie. Brakujący APK nie staje się odpowiedzią SPA.
- Obrazy Linux rzeczywiście zbudowano i uruchomiono. Promtool, Amtool, Loki, Alloy i actionlint sprawdzają konfigurację; lokalny PITR zapisuje wynik w `target/f14-restore-drill.json`.
- Android: testy jednostkowe i cztery instrumentacyjne aplikacji, debug/lint, rzeczywisty login HTTPS i WS, walka/wznowienie, eksport SAF, ograniczona sesja praw oraz poprawna i wadliwa podpisana aktualizacja. Wydanie R8 z kluczem **testowym** uruchamia offline WASM przy wyłączonym debugowaniu WebView. To nie jest klucz ani aplikacja wydawcy.

## Publikacja Androida i otwarte bramki

Chroniony workflow tworzy AAB, APK podpisany właściwym kluczem aplikacji oraz paczkę Ed25519. `prepare-download.mjs` przygotowuje statyczny katalog `downloads/` z plikiem APK, stroną PL/EN, metadanymi i SHA-256. Artefakt należy sprawdzić i opublikować pod `/downloads/`; manifest aktualizacji web publikuje się dopiero po zgodnym serwerze. Klucze przesyłania, podpisu aplikacji i paczek web są rozdzielone.

Do startu pozostają: prawdziwy VPS i domena, Cloudflare/TLS/SMTP/Turnstile, chronione środowiska GitHub, szyfrowany S3 i próba na czystym hoście, odbiór alertu przez właściciela, pomiary F13 i urządzeń, zatwierdzenie prawa/grafik/marki, konta organizacji Play i D-U-N-S, identyfikator aplikacji, kopie kluczy oraz testy ludzi. Beta zamknięta 20–50 osób przez dwa tygodnie i otwarta 2–4 tygodnie nie zostały zastąpione botami. Dopiero ich odbiór pozwala oznaczyć M4/M5A/M6 jako wykonane.

## Wynik końcowy i poprawki przeglądu

Finalna próba `final-runtime-with-notices.log` używała **rzeczywistych finalnych obrazów** serwera i bazy: migracje, `check-config`, start statycznego serwera bez roota, `/healthz`, `/readyz`, publiczne 404 metryk i prywatny poprawny eksport. PITR: 4,80 s od rozpoczęcia odtwarzania, 21,09 s całej lokalnej próby; 26 migracji, właściwy znacznik czasu i zerowe rozbieżności księgi. Nie jest to próba rzeczywistego S3 ani rozmiaru danych produkcji.

Przegląd poprawił również kolejność zwalniania bariery: najpierw zatrzymuje się stary proces, dopiero potem nowy odblokowuje aktywności. Inaczej stary WS mógłby rozpocząć walkę tuż przed zatrzymaniem. Test PITR dostał odstęp od sekundowej daty zakończenia kopii, aby target nie wyprzedzał kopii z tej samej sekundy.

Pakowanie Androida ujawniło pomijanie ukrytego katalogu `.vite` przez Capacitor. APK zawiera teraz jawny `native-release.json`; generator pobierania odczytuje go z faktycznego APK i porównuje aplikację, wersję, SHA wydania, silnik, treść i numer paczki. Identyfikator Capacitor pochodzi z tej samej konfiguracji co Gradle. Testowy podpisany APK i przygotowanie strony przeszły, podpis v2 potwierdził `apksigner`. Strona mobilna przeszła axe (0 naruszeń) i kontrolę zrzutu. Artefakt `target/f14-download-fixture` jest wyłącznie testowy — nie wolno go publikować jako wydania właściciela.

Końcowe kontrole konfiguracji: actionlint, Bash, Promtool (12 reguł), Amtool, Loki i Alloy są zielone. Rzeczywisty test Caddy obejmuje również pobieranie i 404 brakującego APK. Instrukcje pozostają przygotowaniem do uruchomienia przez właściciela, bez fikcyjnego zgłoszenia znaku, zatwierdzenia prawnego, bety ani publikacji Play.

Test instalatora i retencji działa w izolowanym kontenerze z pustym tmpfs, bez dostępu do hosta. Sprawdza ścieżkę `../`, symlinki, niezgodny SHA, kolizję niezmiennego zasobu, zakaz nadpisania oraz zachowanie aktywnego wydania i minimum siedmiu dni. Przegląd ujawnił możliwość dostarczenia fałszywego `.complete` w archiwum; instalator odrzuca teraz ten zastrzeżony plik, a znacznikiem zarządza samodzielnie dopiero po pełnej walidacji.
