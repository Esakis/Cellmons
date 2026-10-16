# F12A — Android, aktualizacje i wydania

Stan implementacji: 2026-10-16. Powłoka, magazyn sesji, integracje natywne, podpisane paczki web i ścieżka budowania są zaimplementowane. **M5A nie jest odebrany:** nie wykonano fizycznej macierzy urządzeń ani publikacji w teście zamkniętym Google Play. Nie wybrano za właściciela trwałego identyfikatora aplikacji, domeny API ani kluczy produkcyjnych.

## Zachowanie aplikacji

Trening WASM działa z zasobów APK, bez internetu. Gość nie wysyła żądań API i nie zapisuje postępu ani ustawień w trwałym magazynie. Natywne ustawienia obejmują wibracje, ekran włączony tylko podczas walki i opcjonalne lokalne przypomnienie po przejściu walki online w tło. Uprawnienie powiadomień jest pytane dopiero po włączeniu tej opcji przez gracza z kontem.

Wyjście w tło zamyka połączenie i wstrzymuje ponowne łączenie. Powrót wznawia sesję. Na prawdziwym backendzie testowym potwierdzono zachowanie walki po 3 s i porażkę przez rozłączenie po 65 s. Przycisk wstecz podczas walki wymaga decyzji gracza; anulowanie zachowuje trening. Zbyt stary WebView pokazuje natywny komunikat, zanim załaduje nieobsługiwany JS.

Token i czas rotacji są szyfrowane AES-256-GCM kluczem Android Keystore. Plik leży w `noBackupFilesDir`, a manifest dodatkowo wyklucza kopie chmurowe i transfer urządzenia. Argumenty mostka nie są logowane, również w debug. Jeden procesowy szereg zapisów porządkuje odczyt po odtworzeniu Activity. Uszkodzenie pliku lub brak klucza wymaga ponownego logowania. Rotacja tokenu unieważnia grant administracyjnego TOTP, zamiast przenosić go na nową sesję.

## Podpisane paczki web

`scripts/pack-web-bundle.mjs` tworzy ZIP, manifest i publiczny klucz. Klucz prywatny jest wskazywany przez `WEB_BUNDLE_SIGNING_KEY` i nie trafia do wyników. Manifest podpisuje dokładne bajty ASCII:

```text
SPELLGAMBIT-WEB-1\n
version\n
min_native_version\n
protocol\n
content_version\n
sha256\n
size\n
```

Każdy wiersz kończy pojedyncze LF, również ostatni. Liczby są dziesiętne bez zer wiodących, skróty to 64 małe znaki hex, podpis to 64 bajty Ed25519 zapisane Base64. Klucz w APK to 32 surowe bajty publiczne w Base64. Paczka jest pobierana wyłącznie z `<WEB_BUNDLE_URL>/<version>.zip`; manifest nie może wskazać innej domeny.

Weryfikator używa utrzymywanej biblioteki [Bouncy Castle 1.86](https://www.bouncycastle.org/resources/new-release-bouncy-castle-java-1-86/). Sprawdza podpis, protokół, minimum powłoki, rozmiar i SHA-256 przed rozpakowaniem. Limity: ZIP 10 MiB, dane po rozpakowaniu 40 MiB, 5000 wpisów; odrzucane są przejścia poza katalog, powtórzenia ścieżek i pliki spoza listy zasobów web. Nie pobiera kodu natywnego. HTTPS zachowuje systemową weryfikację certyfikatów i nie śledzi przekierowań.

Sprawdzenie następuje po rozpoznaniu konta i co 6 h; po `VERSION_MISMATCH` można wymusić je najwyżej raz na 5 min. Gość nie wykonuje sprawdzeń. Aktywacja czeka na znaną sesję, brak walki/aktywności i aplikację na pierwszym planie. Nowa paczka musi uruchomić UI i rzeczywisty trening WASM oraz potwierdzić start. Watchdog liczy 15 s na pierwszym planie. Brak potwierdzenia lub restart procesu podczas próby przywraca poprzednią potwierdzoną paczkę, ewentualnie wbudowaną. Nieudana wersja nie jest ponawiana; naprawa wymaga wyższego numeru. Rotacja publicznego klucza w APK odrzuca wcześniejsze zapisane paczki.

Brak `WEB_BUNDLE_URL`/`WEB_BUNDLE_PUBLIC_KEY` wyłącza pobieranie. Nowy APK określa minimalny dopuszczalny numer przez `WEB_BUNDLE_VERSION`. Zachowanie aktywności pomiędzy różnymi wydaniami serwera wymaga także procedury zachowania starszych workerów i zasobów z F14; sam podpis nie rozwiązuje zgodności silników.

## Budowanie i klucze

Debug: `pnpm --filter mobile sync`, następnie Gradle `assembleDebug`. Wymagane JDK 21 i SDK Androida. `minSdk=26`, `targetSdk=36`. Identyfikator debug to `dev.game.training`.

Release wymaga `ANDROID_APPLICATION_ID`, `ANDROID_VERSION`, rosnącego `WEB_BUNDLE_VERSION`, `VITE_API_ORIGIN`, obu ustawień podpisanej paczki oraz `ANDROID_KEYSTORE`, `ANDROID_STORE_PASSWORD`, `ANDROID_KEY_ALIAS`, `ANDROID_KEY_PASSWORD`. Wykonać `mobile sync` z tym samym środowiskiem przed Gradle. Kontrola porównuje metadane dołączonych zasobów z konfiguracją powłoki; nie pozwala podpisać przypadkowo starego klienta. Kod wersji to `major*10000+minor*100+patch`.

`.github/workflows/android.yml` buduje debug w PR i uruchamia nocny emulator. Release jest ręczny, z `main`, korzysta ze środowiska `release`. **W ustawieniach GitHub właściciel musi skonfigurować wymaganych recenzentów i dozwolone gałęzie tego środowiska.** Sam zapis YAML nie ustanawia tych zabezpieczeń.

Sekrety środowiska: `ANDROID_APP_KEYSTORE_BASE64`, `ANDROID_APP_STORE_PASSWORD`, `ANDROID_APP_KEY_ALIAS`, `ANDROID_APP_KEY_PASSWORD`; analogiczne `ANDROID_UPLOAD_*`; `WEB_BUNDLE_SIGNING_KEY_PEM`. Zmienne środowiska: trwały ID, adres API, URL paczek, publiczny klucz. AAB podpisuje osobny klucz przesyłania; APK klucz podpisu aplikacji. W Play App Signing trzeba zarejestrować ten sam własny klucz aplikacji, aby APK i instalacja z Play miały zgodne podpisy. Kopie offline kluczy w dwóch niezależnych miejscach pozostają zadaniem właściciela.

Powstał lokalny podpisany **fixture**, `dev.game.training.fixture`, z jednorazowym kluczem testowym, bez publikacji. Po R8 miał APK 1 344 364 B i AAB 1 739 618 B; podpis v2 zweryfikowano `apksigner`. To pomiar wcześniejszego wariantu testowego, nie finalne artefakty produkcyjne. Natywny test dostępności uruchomił jego trening offline przy wyłączonym debugowaniu WebView. Zaufanie do lokalnego CA istnieje wyłącznie w debug i nie występuje w release.

## Weryfikacja i odtwarzanie testów

- 6 testów JVM: rzeczywisty Ed25519, zmiana manifestu, hash/rozmiar, bezpieczne rozpakowanie, niezgodność, stan próby i rollback.
- 4 testy instrumentowane Android 14: szyfrowanie, odtworzenie i czas rotacji, manipulacja plikiem, reset po rotacji klucza paczek.
- `e2e/android-smoke.mjs`: gość, brak API i trwałych zmian preferencji, trening bez sieci, ekran, tło, przycisk wstecz.
- `e2e/android-online.mjs`: lokalny backend z osobną bazą, prawdziwy TLS, logowanie, Keystore, bilet WS, powrót przed i po 60 s, wylogowanie.
- `e2e/android-updates.mjs`: poprawna paczka, uszkodzone archiwum, błędny podpis, minimum nowszej powłoki, nieudany start i powrót do poprzedniej wersji.
- Test PG `incompatible_workers_do_not_claim_or_extend_other_releases`: worker nie przejmuje ani nie odnawia dzierżawy obcej wersji silnika/treści; zgodną wersję przejmuje.

`node e2e/android-fixture.mjs` przygotowuje krótkotrwały lokalny certyfikat i klucz testowy. Ustawić `VITE_API_ORIGIN=https://10.0.2.2:18443`, `E2E_TLS_CERT` na wygenerowany certyfikat, `WEB_BUNDLE_URL=https://10.0.2.2:18443/app-bundles/` oraz wygenerowany klucz publiczny; dopiero wtedy synchronizować i budować debug. **Testy instrumentowane odinstalowują aplikację docelową podczas sprzątania — po nich wykonać osobno `installDebug`.** Testy E2E wymagają emulatora przeznaczonego do zadania. Nie używają realnych kont; test aktualizacji czyści dane pakietu debug. Nie uruchamiać równolegle z web E2E, które używa tego samego portu fixture.

Surowe wyniki lokalne są w ignorowanym `target/f12a-*`. Test emulatora nie zastępuje pomiaru baterii, FPS, pamięci i zimnego startu na telefonie. Budżet PWA 200 KiB pozostaje niezmieniony; Android pakuje wszystkie zasoby, także online. Osobne limity: zasoby web 10 MiB bez kompresji i APK 20 MiB. Obecny zestaw zasobów zajmuje ok. 1,63 MiB.

## Odbiór przed publikacją

Macierz: Android 8–10 / 2–3 GB RAM, średni Android 12–13, Android 14+, tablet. Dla każdego zapisać model, wersję WebView, zimny start, pamięć podczas 30 min gry, FPS, spadek baterii, zachowanie klawiatury/czatu i TalkBack. Zmierzyć LTE/Wi-Fi oraz zmianę sieci podczas walki. Nie wpisano fikcyjnych pomiarów.

`mobile/store/` zawiera teksty PL/EN, własne grafiki i roboczą inwentaryzację formularzy. Konta Play, raportu przedpremierowego, testerów, trwałych URL-i prawnych, zatwierdzenia grafik i deklaracji IARC nie wykonano bez danych właściciela. Eksport/usuwanie konta oraz przegląd prawny są bramką F13. Materiały nie zostały opublikowane.

Końcowy przegląd lokalny: w starym emulatorze 32.1.15 wystąpił ANR w rendererze WebView/OpenGL podczas przewijania przez UiAutomator. Usunięto zbędny drugi kontroler ekranu startowego (pozostaje motyw uruchomienia Capacitor), test przestał korzystać z nieaktualnego drzewa dostępności, a emulator uruchomiono bez Vulkanu. Trzy kolejne uruchomienia podpisanego APK zakończyły trening offline poprawnie. Nie jest to diagnoza wszystkich sterowników telefonów; fizyczna macierz F12A.6 nadal wymaga odbioru. CI korzysta z aktualnego trybu `swiftshader`, zgodnie z [dokumentacją emulatora](https://developer.android.com/studio/run/emulator-acceleration).

Weryfikacja końcowa części lokalnej: wszystkie kontrole Rust/AI/WASM i 98 testów PostgreSQL przeszły. Pierwszy pełny przebieg wykrył moduł Androida omyłkowo dodany do PWA (202,4/200 KiB). Po wyraźnym rozdzieleniu importu platformy paczka ma 199,6/200 KiB, modułu natywnego nie ma w buildzie web; ponownie przeszły 26 Vitest, typy wszystkich pakietów, 24 Playwright i Lighthouse 100/100/100/100. Po ostatniej poprawce typów artefakty web są bajtowo identyczne z przetestowanymi. Biome jest zielony. Audyt Node po aktualizacji zależności narzędziowej `xcode>uuid` do 11.1.1 nie zgłasza podatności.
