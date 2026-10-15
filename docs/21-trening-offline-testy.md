# F5 — trening offline i weryfikacja

Stan z 15.10.2026. Klient przeglądarkowy jest grywalny. Powłoka Android debug działa w emulatorze. Kryteria pomiarowe na fizycznych telefonach pozostają otwarte.

## Dostarczony zakres

- Routing przez `HashRouter`, polski i angielski w Fluent, wspólne tokeny jasnego/ciemnego motywu, modal z natywnym zarządzaniem fokusem. Zatwierdzone portrety i ikony SVG z `art/` przechodzą przez SVGO; lokalne podzbiory Inter mają licencję OFL.
- Sześć postaci i ich prawdziwe talie startowe, cztery profile AI z rotacją postaci oraz tryb obserwacji botów. Trening używa poziomu 1. Nie wymaga konta ani serwera i nie przyznaje nagród.
- Rust/WASM działa w leniwie tworzonym Web Workerze. UI przesyła Protobuf jako przenoszone `ArrayBuffer`. Worker przechowuje stan pojedynku; odpowiedzi zawierają tylko widok gracza i jawne zdarzenia. Nie zwracają ziarna, talii ani dobieranych kart przeciwnika.
- Podgląd kosztów, legalność rzucenia każdej karty i rozstrzygnięcie pochodzą z silnika Rust. Obsługa klawiatury, myszy i dotyku, inspekcja karty, odrzucanie, inicjatywa, szkoły i umagicznienia. W treningu zegar pokazuje czas namysłu bez limitu; zegary i walkowery serwera dotyczą gry online.
- Sekwencja ujawnienia ruchów, czarów i ataków, aktualizacja widocznych zasobów, log zdarzeń, pomijanie i powtórka ostatniej rundy, wynik i ponowny trening. Animacje respektują ustawienie ograniczonego ruchu.
- PWA zapisuje cały pakiet, w tym nieotwarty jeszcze ekran walki, worker, WASM, czcionki i treść. Nie buforuje API, WebSocket ani kontroli stanu serwera. Aktualizacja wymaga bezczynności wszystkich otwartych kart; nie przeładowuje aktywnego pojedynku. Odświeżenie strony zaczyna nowy trening; stan pojedynku nie jest zapisywany.

## Weryfikacja

Końcowy wynik lokalny: **18/18 etapów `check.bat` (122 s), 14/14 scenariuszy Playwright, 7/7 testów Vitest**, bez pominięć. APK debug zbudowany przez `assembleDebug`: ok. 4,5 MB.

Pełny `check.bat` obejmuje Rust, clippy, walidację treści, balans postaci, cztery serie AI, testy natywne/WASM w Node, typy, Vitest, budowanie, Protobuf, Playwright, Lighthouse i serwer z PostgreSQL. Test protokołu w Rust rozgrywa cały pojedynek botów, sprawdza niezmienność stanu po podglądzie i powtórce, prywatność dobierania oraz odrzucanie błędnych żądań. Test tłumaczeń formatuje opisy wszystkich 72 kart w obu językach bez nierozwiązanych zmiennych.

Playwright sprawdza pełny trening gracza, pełny pojedynek botów po odłączeniu sieci i przeładowaniu strony, skróty, koszty, modal, poddanie, animację, powtórkę, jawne akcje kart, dotyk i brak poziomego przewijania. Scenariusze działają w Chromium na komputerze i w emulacji Pixel 5. Emulacja nie zastępuje pomiarów na telefonach.

Lighthouse mobile na zbudowanej aplikacji: **100 Performance, 100 Accessibility, 100 Best Practices, 100 SEO**. Polecenie `pnpm audit:web` uruchamia własny serwer podglądu i zapisuje raport w `target/lighthouse/`. Wszystkie cztery kategorie muszą mieć co najmniej 95. To pomiar laboratoryjny na tym komputerze.

Budowanie automatycznie sprawdza budżety z `13-wydajnosc.md`:

| Zasób | Pomiar F5 | Limit |
|---|---|---|
| Początkowy JS, Brotli | ok. 31 KiB | 70 KiB |
| Początkowy CSS, Brotli | ok. 4 KiB | 20 KiB |
| Leniwy ekran walki, JS Brotli | ok. 23 KiB | 40 KiB |
| Czcionki WOFF2 łącznie | 24,3 KiB | 40 KiB |
| Silnik WASM, Brotli | 68,2 KiB | 90 KiB |
| Cały pakiet offline, kod Brotli, obrazy i fonty bez dodatkowej kompresji | ok. 174 KiB | 200 KiB |

Profile botów są kompilowane z TOML podczas budowania. Test porównuje je z walidowanym źródłem. Usuwa to parser TOML z wynikowego WASM i zmniejsza go z ok. 136 do 68 KiB Brotli, bez zmiany decyzji AI. Rzeczywista kompresja HTTP zależy od hostingu; lokalny serwer podglądu służy do testów funkcji, a rozmiary Brotli liczy skrypt budowania.

## Android — F5.6

`mobile/` zawiera projekt Capacitor 8.5.3 i Android/Gradle, `minSdk=26`, `compileSdk=targetSdk=36`. Budowanie wymaga JDK 21 i Android SDK. Tymczasowe ID debug to `dev.game.training`; identyfikator do publikacji pozostaje decyzją F12A. Zasoby są w APK, rejestracja Service Workera jest wyłączona. Ikony i splash powłoki są na razie z szablonu Capacitor; branding wydania należy do F12A.

Przeprowadzone sprawdzenie: emulator Pixel 3a, Android 14/API 34, x86_64, WebView 113.0.5672.136. `assembleDebug` i instalacja APK zakończyły się poprawnie. Po wyłączeniu Wi-Fi i transmisji danych aplikacja została przeładowana i rozegrała pełny pojedynek botów przez WASM do ekranu wyniku. Liczba zarejestrowanych Service Workerów: 0. Sterowanie testem przez lokalny protokół debugowania WebView, bez zmian w kodzie gry.

`adb shell am start -W` po zatrzymaniu procesu: pierwsze uruchomienie po instalacji **3408 ms**, kolejne **1524 ms** i **1074 ms**. To czas startu aktywności emulatora, nie pomiar gotowości interfejsu na urządzeniu. Pierwszy wynik przekracza cel 2 s; nie uznajemy kryterium startu za zamknięte. Nie zmierzono 60 FPS, pamięci ani baterii na fizycznym sprzęcie.

Do zamknięcia F5.4/F5.6: instalacja debug na 2–3 fizycznych telefonach z macierzy w `19-aplikacja-android.md`, pomiar zimnego startu do interaktywności i płynności, kontrola dotyku, skalowania, TalkBack i zachowania po przejściu w tło. Ryzyko RY-10 pozostaje otwarte.

## Uruchamianie

Przeglądarka: `run.bat`. Do weryfikacji instalacji PWA/offline: `pnpm --filter web build`, następnie `pnpm --filter web preview` i wejście na localhost:4173. Poczekaj na „Gotowe do gry offline”. Service Worker działa na HTTPS lub localhost; serwer deweloperski nie instaluje PWA.

Android: `pnpm --filter mobile sync`, następnie `mobile/android/gradlew.bat -p mobile/android assembleDebug` z katalogu głównego repozytorium. Ustaw `JAVA_HOME` na JDK 21 i `ANDROID_HOME` na Android SDK. APK: `mobile/android/app/build/outputs/apk/debug/app-debug.apk`. Po synchronizacji Androida ponów zwykłe budowanie web przed testami PWA — wariant Android nie zawiera Service Workera.

Kod TypeScript protokołu jest zapisany w repozytorium: `pnpm proto:generate` po zmianie `.proto`. Rust generuje typy przez `prost-build` z dołączonym kompilatorem `protoc`. CI sprawdza aktualność typów TS. Nowe zależności produkcyjne: router, Fluent, Protobuf; Capacitor tylko w powłoce Android. SVGO, generatory i Lighthouse służą wyłącznie do budowania lub kontroli jakości.
