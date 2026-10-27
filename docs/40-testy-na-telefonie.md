# Testy Androida z lokalnym serwerem i kontami

`phone.bat` przygotowuje testowy APK połączony z trwałą lokalną bazą i 40 kontami sterowanymi przez serwer. Telefon łączy się przez domową sieć z komputerem; komputer może być podłączony do routera kablem.

## Uruchomienie

1. Podłącz telefon do tej samej sieci lokalnej co komputer.
2. Uruchom `phone.bat`. Pierwsze uruchomienie buduje APK, kolejne używają przygotowanego pliku.
3. Otwórz na telefonie adres wypisany przez skrypt, np. `http://192.168.33.103:18080`.
4. Pobierz i zainstaluj `Spellgambit-test.apk`.
5. Dane konta testowego znajdują się wyłącznie na komputerze w `target/phone-test/konto-testowe.json`.
6. W aplikacji wybierz Konto, zaloguj się, otwórz Lobby i Arenę. Wybierz wolnego przeciwnika z poziomem zbliżonym do twojego i wyślij wyzwanie.

Większość z 40 kont jest obecna na Arenie. Część może być w pojedynku lub odwiedzać inne lokacje. Zwykła walka z nimi zapisuje postęp gracza. Gość i trening offline nie łączą się z tymi kontami.

Konto testowe podlega tym samym zasadom co każde inne. Rejestruje się jako Mag z gotową talią 20 kart, a kolejne klasy zdobywa na poziomach konta 5, 8, 12… albo przez kontrakt. Nie ma skryptu przyznającego mu całą pulę. Zwykła rejestracja w aplikacji daje jedną klasę wybraną z trzech: Mag, Wojownik lub Kapłan, a następnie prowadzi przez samouczek. Pierwszy ukończony trening otwiera Arenę na poziomie konta 3; pozostałe lokacje mają własne progi.

Konto przygotowane wcześniej skryptem `prepare-phone-roster.mjs` zachowuje w lokalnej bazie przyznane klasy. Aby zacząć od zera, usuń `target/phone-test/konto-testowe.json` i uruchom `phone.bat`; skrypt utworzy nowe konto `Telefon_*`.

Komputer i Docker muszą działać podczas gry. Po ponownym uruchomieniu komputera wykonaj `phone.bat`. Zatrzymanie: `phone.bat -Stop`; dane pozostają. Po zmianach źródeł wykonaj `phone.bat -Rebuild`: przebuduje i uruchomi ponownie własny serwer oraz przygotuje nowy APK. Skrypt odmawia zatrzymywania serwera uruchomionego poza nim. Zainstaluj ponownie przygotowany APK na telefonie, aby otrzymać nowy interfejs.

## Adres i połączenie

Skrypt wybiera lokalny adres IPv4 z domyślnej trasy. Można wskazać go jawnie: `phone.bat -Address 192.168.33.103`. Zmiana adresu lub certyfikatu przebudowuje APK; wtedy zainstaluj nowy plik na telefonie. Dla wygody warto zachować rezerwację adresu komputera w routerze.

API i WebSocket używają HTTPS/WSS na porcie 18443. Lokalny certyfikat jest dołączony wyłącznie do debugowego APK, korzystając z istniejącej konfiguracji Androida. Telefon nie wymaga instalowania systemowego certyfikatu ani wyłączania weryfikacji TLS. Mechanizm opisuje [dokumentacja Network Security Configuration](https://developer.android.com/privacy-and-security/security-config).

Port HTTP 18080 udostępnia tylko stronę instrukcji i jeden plik APK. Nie publikuje katalogu projektu, haseł, poczty ani bazy. Logowanie i walka odbywają się na osobnym porcie HTTPS. Reguła zapory obejmuje tylko te dwa porty procesu Node, prywatny profil sieci i lokalną podsieć.

Jeżeli reguły zapory nie można dodać bez podwyższonych uprawnień, skrypt wyświetla komunikat. W takim przypadku uruchom `phone.bat` raz jako administrator. Sieć gościnna lub izolacja klientów Wi-Fi może blokować połączenia z komputerem. Ten wariant nie udostępnia gry przez internet ani transmisję komórkową.

## Pliki

| Plik | Przeznaczenie |
|---|---|
| `target/phone-test/Spellgambit-test.apk` | APK do instalacji, osobny pakiet `dev.game.training.phone` |
| `target/phone-test/konto-testowe.json` | Login i hasło jednego konta testującego; ponowienie zachowuje konto |
| `target/phone-test/connection.json` | Adresy i ważność certyfikatu |
| `target/phone-test/build.json` | Konfiguracja użyta do zbudowania APK |
| `target/phone-test/server.stdout.log` i `server.stderr.log` | Logi serwera gry |
| `target/phone-test/gateway.stdout.log` i `gateway.stderr.log` | Logi połączenia telefon–komputer |

Katalog `target` jest ignorowany przez Git. Klucz TLS i hasło konta nie trafiają do APK ani strony pobierania. Użytkownik może także zalogować się własnym kontem z tej lokalnej bazy.

Wymagania budowania: narzędzia z `CONTRIBUTING.md`, Android SDK i JDK co najmniej 21. Skrypt sprawdza `JAVA_HOME`, JDK Android Studio i lokalne `target/android-tools/jdk-*`. Działający serwer na porcie 8090 jest używany ponownie; powinien być uruchomiony z `LOBBY_BOTS_ENABLED=true` i tą samą lokalną bazą. Zatrzymywanie kończy wyłącznie procesy zapisane przez skrypt, z kontrolą czasu startu i ścieżki.

Aktualne zasady poziomów konta, odblokowań i inkarnacji znajdują się w [dokumencie progresji](41-plan-progresji-i-sciezek.md). Dokument oddziela wdrożone mechaniki od propozycji przyszłych nazw specjalizacji.

## Weryfikacja z 20 października 2026

Aktualny APK 1.0.1 ma **12 855 462 bajty**, poprawny podpis v2 i SHA-256 `fdfa5068ef52501f1fd4aa53300660a774a89014a48c5c76aa73cad85120ac93`. Sprawdzono certyfikat HTTPS, gotowość bazy i identyczność pobranego APK z lokalnym plikiem. Pliki konta, klucze i katalog projektu nie są udostępniane przez serwer pobierania.

Na emulatorze Androida 14 zaliczono osiem kontroli: logowanie natywne przez HTTPS, zapis tokenu w Keystore, wybór ośmiu klas, talia Wampira, obecność kont na Arenie, przyjęcie wyzwania, pełny pojedynek oraz brak poziomego przewijania. Na Arenie było 29 kont; Wampir stoczył z LunaVey dwurundową walkę zakończoną wynikiem zapisanym przez serwer. Wszystkie 40 kont ma aktywną postać z wydanej ósemki; sześć dawnych Runotechników ma nowych aktywnych zastępców, z zachowaniem starych danych.

Scenariusz `node e2e/android-phone.mjs` wymaga konta z przygotowaną ósemką i już uruchomionej aplikacji w przeznaczonym do testu emulatorze `emulator-5580`. Odmawia pracy na fizycznym telefonie. Raport i zrzuty, w tym `android-vampire.png`, znajdują się w `target/phone-test/`.

Przeszły 26 scenariuszy Playwright (komputer i widok mobilny), 123 testy PostgreSQL, 38 testów klienta, pełne `cargo test --workspace --locked`, kontrola typów i lint. Sprawdzono walki wszystkich wydanych klas oraz dróg w silniku i WASM, blokady pozostałych klas oraz jednorazowe pakiety. Lighthouse: wydajność 95, dostępność 100, dobre praktyki 100, SEO 100. Macierz 28 000 walk talii startowych spełnia próg maksymalnie 60% średniego wyniku klasy, zachowując kontry.

**Otwarte strojenie AI:** przy nowej puli test 1200 walk wykazał 58,1% zwycięstw domyślnego L2 przeciw L1 (próg 60%) i 52,7% profilu Arcymaga przeciw Mistrzowi Bitew (próg 55%). Pozostałe dwa porównania i limit czasu decyzji przeszły. Dlatego całego `check.bat` nie należy przedstawiać jako zielonego; działanie walk potwierdzono, ale hierarchia trudności wymaga dalszej korekty. Szczegóły: `target/roster-check2.log`, wyniki poprawionych testów: `target/roster-e2e-final.log`, `target/roster-database.log`, `target/roster-online.log`, `target/roster-remaining.log`, `target/roster-final-database.log`. Dwa wcześniejsze błędy w `roster-database.log` dotyczą testów poprawionych i zaliczonych następnie w `roster-online.log`, a wcześniejsze błędy turniejów poprawiono w `roster-final-database.log`.

Dodatkowego scenariusza odtwarzania sesji po restarcie nie uznano za zaliczony: podczas prób proces emulatora QEMU kończył się awarią Windows `0xc0000005`. Powrót do aplikacji i zachowanie sesji trzeba jeszcze sprawdzić na fizycznym telefonie. Testy na emulatorze nie potwierdzają również połączenia przez konkretny domowy router.

## Aktualizacja po uwagach z telefonu — 2026-10-20, APK 1.0.2

Poprawiono mobilny kafel „Twój ruch. Twoja magia.” i ułożono mapę w nieruchomą siatkę. Główny czat automatycznie dołącza do Rynku; wysyłanie ma stan oczekiwania i czytelne błędy ograniczeń prywatnych wiadomości. Nowe konta wybierają Maga, Wojownika lub Kapłana. Samouczek odsłania kolejne lokacje, a trwały poziom konta steruje blokadami także na serwerze. Edytor talii pokazuje miniatury, podgląd czaru, filtry, cofnięcie zmiany i próbną rękę. Szczegóły progów i osobną propozycję zdobywania kolejnych postaci zawiera [plan progresji](41-plan-progresji-i-sciezek.md).

Weryfikacja tej aktualizacji:

- 73 testy PostgreSQL z grup konta i lobby, a po ostatniej korekcie ponownie trzy testy progresji konta.
- 12 scenariuszy Playwright na komputerze i w widoku mobilnym: rejestracja, pierwszy trening i awans konta, czat, poczta, talie oraz istniejące funkcje online.
- Dodatkowy przebieg mobilny sprawdził polskie i angielskie menu przy 320 × 640, 390 × 844 i 412 × 915, bez pionowego i poziomego przewijania głównej mapy po samouczku.
- Kontrola typów klienta i testów, lint zmienianego interfejsu oraz budżety pakietów przeszły. Zmiany dwóch budżetów są jawnie opisane w [dokumencie wydajności](13-wydajnosc.md).
- Zbudowano APK `dev.game.training.phone`, wersja 1.0.2 / 10002, 12 761 403 bajty; SHA-256 `aaf642085a1c05564f20023d10227976ecf71ab0a6a475026fe221a85e5ff91c`. Lokalny serwer został przebudowany, migracja 29 zastosowana, a kontrola TLS, gotowości bazy i pobierania APK przeszła.

Logi: `target/mobile-feedback-server-tests.log`, `target/mobile-feedback-journey-tests.log`, `target/mobile-feedback-e2e-final.log`, `target/mobile-feedback-e2e-polish.log`, `target/mobile-feedback-phone-build.log`. Zrzuty: `target/phone-home-320.png`, `target/phone-menu-pl-320.png`, `target/phone-menu-level3.png`, `target/deck-editor-mobile-chromium.png`.

Podczas tej aktualizacji nie było podłączonego urządzenia ADB; nowego APK nie przetestowano na fizycznym telefonie ani ponownie w emulatorze. Zainstaluj nowy plik z dotychczasowej strony pobierania. Dane starego konta testowego pozostały zachowane; pełny start z wyborem jednej z trzech klas sprawdza się na nowym koncie.

## Rekrutacja postaci — 2026-10-20, APK 1.0.3

Gwarantowane nagrody są już dostępne w **Kolekcji → Rekrutacja postaci** (także przez Bibliotekę na mapie). Poziomy konta 5, 8, 12, 16, 20, 24 i 28 pozwalają wybrać nieposiadaną klasę, obejrzeć jej rolę i talię, a następnie odebrać gotową postać z pakietem kart. Obecna aktywna postać pozostaje aktywna. Pełne miejsca zatrzymują odbiór bez utraty nagrody; po zdobyciu wszystkich klas nagrodą jest ozdobna pieczęć. Stare konto testowe zachowuje swoją ósemkę i otrzymuje pieczęcie po osiągnięciu odpowiednich progów, bez resetowania postępu.

Przeszło 47 testów PostgreSQL (39 konta i 8 prywatności), w tym progi, nieaktywne i już posiadane klasy, wyścigi odbiorów, ponowienia, usunięcie postaci, pełne miejsca i pieczęcie. Rozszerzone testy eksportu oraz wymazania historii nagród również przeszły. Dziesięć scenariuszy Playwright potwierdziło działanie konta, kolekcji i funkcji online na komputerze i telefonie, łącznie z prawdziwym odbiorem Maga, talią 20 kart i ponowieniem bez duplikatów. Po skróceniu panelu powtórzono cały scenariusz mobilny; polski ekran przy 320 px i kontrola dostępności przeszły.

Zbudowany APK ma wersję **1.0.3 / 10003**, rozmiar 12 767 128 bajtów i SHA-256 `bba82a930fe5f07538549e41e3da19fda4f08356e2213518f8f992431bbe3665`. Serwer lokalny został przebudowany, migracja 30 zastosowana, a kontrola TLS, gotowości bazy i pobrania APK przeszła. Typy, lint i budżety pakietów także przeszły. Ta aktualizacja ma testy przeglądarkowe i kompilację Androida; nie wykonano nowego testu na fizycznym telefonie.

Logi: `target/recruitment-server-tests.log`, `target/recruitment-export-test.log`, `target/recruitment-erasure-test.log`, `target/recruitment-e2e.log`, `target/recruitment-mobile-final.log`, `target/recruitment-budget.log`, `target/recruitment-phone-build.log`. Zrzuty: `target/recruitment-pl-320.png` i `target/recruitment-preview-mobile-chromium.png`.

## Pełny pakiet progresji i przegląd — 2026-10-20, APK 1.0.4

Nowy APK zawiera wszystkie poprzednie poprawki telefonu oraz kontrakty na postacie, opanowanie kart, zadanie dzienne, dwie inkarnacje i osobną Arenę rankingową. Start to jedna postać: Mag, Wojownik albo Kapłan. Wprowadzenie daje 100 PD konta, pierwszy trening 150 PD; łącznie otwierają Arenę na poziomie 3. Edycja talii otwiera się na poziomie konta 2, kontrakty i zadanie dzienne na 4, opanowanie kart i zestawy na 7. Próby srebra i złota wymagają konta 12/18 oraz postaci 20; ranking i utworzenie klanu — konta 15.

Kontrakt wymaga 30 fragmentów i pozwala wybrać brakującą klasę niezależnie od nagród poziomowych. Opanowanie kart 1–5 jest wspólne dla kopii i daje odznaki oraz małe nagrody w Dukatach. Srebro i złoto przyznają łącznie 3/6 trwałych PR; stare talie trafiają do archiwum, a pakiet drogi jest jednorazowy. Migracja 31 zachowuje stare postacie, przydzielone parametry i dary. W rankingu parametry bazowej klasy są wyrównane do poziomu 20, a talia pozostaje własna.

Przegląd kodu i testów opisuje [raport aktualizacji](42-przeglad-progresji-i-telefonu.md), łącznie z poprawionymi błędami i otwartym problemem hierarchii botów. Zaliczone: 135 testów PostgreSQL, 38 testów klienta, wszystkie 26 scenariuszy przeglądarkowych w przebiegu głównym i uzupełniającym, testy workspace i WASM, Clippy, typy, Biome oraz SQLx. Lighthouse w pełnym przebiegu: 97/100/100/100. Nie oznacza to zielonego `check.bat`: dwa porównania siły AI nadal nie spełniają progów.

APK: **1.0.4 / 10004**, pakiet `dev.game.training.phone`, **12 772 317 bajtów**, poprawny podpis v2. SHA-256: `e447916d48b1045b03450857d6fda7131a04b9de13000cb0764e3f8b6534292d`. Lokalny backend przebudowano, zastosowano migrację i uruchomiono 40 dotychczasowych kont automatycznych. Sprawdzono TLS, gotowość bazy oraz identyczność pobranego APK. Konto testowe i jego dane zachowano.

Pobieranie w tej samej sieci: `http://192.168.33.103:18080`. Zainstaluj nowy APK. Nie było podłączonego urządzenia ADB; tej wersji nie sprawdzono na fizycznym telefonie. Pełny samouczek i wybór jednej klasy należy oglądać na nowym koncie, ponieważ stare konto testowe zachowuje swoją kolekcję.

Log budowania: `target/advancement-phone-build.log`. Końcowy budżet przeglądarki: `target/advancement-budget-final.log` — konto/lobby 55,325/56 KiB, rekrutacja i kontrakty 6,195/10 KiB, pełny pakiet offline 207,733/210 KiB. Zrzuty: `target/advancement-pl-320.png`, `target/reincarnation-mobile-chromium.png`, `target/phone-menu-pl-320.png`.
