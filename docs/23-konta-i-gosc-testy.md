# F6.2 — konta, logowanie i lokalny gość

Stan z 15.10.2026. Zaimplementowano API kont i formularze PL/EN oraz wymagany przez właściciela tryb gościa bez konta, połączenia z API i zapisu danych. Następny krok F6.3 doda gateway WebSocket.

## Konto i gość

Rejestracja przyjmuje login, e-mail, hasło i szkołę. Jedna transakcja tworzy konto, odblokowanie szablonu, aktywną postać poziomu 1, dokładną kolekcję i talię z `engine-content::STARTERS`, ranking 1000 oraz token potwierdzenia adresu. Błąd dostarczenia wiadomości przy rejestracji wycofuje całą transakcję. Wybór szkoły pozostaje stały. Samo utworzenie konta nie tworzy sesji — następnie gracz się loguje.

Hasła mają 10–128 znaków, limit 512 bajtów i sprawdzenie na dołączonej liście popularnych haseł. Hash to Argon2id v19, 19 MiB, 2 przebiegi, 1 pasmo, z losową solą. Hashowanie pracuje poza wątkami Tokio i ma limit 4 równoległych zadań. Nieznany login przechodzi weryfikację zastępczego hasha; błąd nie ujawnia istnienia konta.

„Graj jako gość” działa wyłącznie w pamięci frontendu. Nie tworzy użytkownika, sesji, tokenu ani rekordu w bazie. Blokuje wywołania API, zapis ustawień i odpytywanie panelu stanu deweloperskiego. Dane i wybory nie trafiają do localStorage, sessionStorage, IndexedDB ani ciasteczek. Odświeżenie lub zamknięcie strony usuwa gościa. Aktualnie dostępny jest trening offline, który zgodnie z R-TRE-01/R-PD-03 nie przyznaje nagród ani PD. Pliki samej aplikacji mogą pozostać w cache PWA, aby działała offline. Nie ma automatycznego przenoszenia stanu gościa do konta.

Konto z niepotwierdzonym e-mailem jest trwałym kontem z R-KNT-02. Nazwę gościa i zmianę wcześniejszego znaczenia dokumentuje ADR-018; gość lokalny jest osobnym trybem R-KNT-04.

## Sesje i API

| Operacja | Kontrakt |
|---|---|
| `POST /api/auth/register` | `{login,email,password,school,client?,captcha_token?}` → 201 `{registered:true}` |
| `POST /api/auth/login` | `{identity,password,client?:"web"|"android"}` → `{profile}`; Android także `{token,expires_in}` |
| `GET /api/me` | profil konta i aktywna postać |
| `POST /api/auth/logout` | usuwa bieżącą sesję; web usuwa ciasteczko |
| `GET /api/auth/sessions` | lista własnych, niewygasłych sesji |
| `DELETE /api/auth/sessions/{id}` | zakończenie sesji należącej do bieżącego konta |
| `POST /api/auth/verify-email` | `{token}`; link ważny 24 h, jednorazowy |
| `POST /api/auth/verify-email/request` | `{email}`; zastępuje poprzedni link |
| `POST /api/auth/password-reset/request` | `{email}`; odpowiedź nie ujawnia istnienia konta |
| `POST /api/auth/password-reset/confirm` | `{token,password}`; link ważny 30 min, jednorazowy; unieważnia wszystkie sesje i pozostałe tokeny e-mail |
| `POST /api/auth/rotate` | Bearer aplikacji → nowy token; stary natychmiast przestaje działać |
| `POST /api/auth/ws-ticket` | Bearer aplikacji → `{ticket,expires_in:30}` |
| `GET /api/auth/options` | publiczny klucz Turnstile albo `null` w środowisku lokalnym |

Web dostaje ciasteczko `__Host-game_session; HttpOnly; Secure; SameSite=Lax; Path=/`, bez `Domain`. Tylko lokalne HTTP używa nazwy `game_session` bez `Secure`. Token sesji to 256 losowych bitów, w bazie jest wyłącznie SHA-256. Nowe logowanie web zastępuje identyfikator z bieżącego ciasteczka. Sesje wygasają po 30 dniach; użycie sesji przedłuża termin w bazie. Konto ma maksymalnie 20 aktywnych sesji. Ban lub zmiana statusu konta blokuje uwierzytelnianie.

Android używa `client=android`, `X-Native-Version` i później `Authorization: Bearer …`; nie otrzymuje ciasteczka. CORS zezwala wyłącznie na skonfigurowane `APP_ORIGINS`, bez credentials. `Retry-After` jest dostępny dla klienta CORS. Bilet WebSocket zawiera hash sesji, termin i losowy nonce, ma podpis HMAC-SHA256 i jest jednorazowy. `Auth::consume_ws_ticket` sprawdza pochodzenie, podpis, termin, wydanie przez bieżący proces i ważność sesji; czeka na użycie w gateway F6.3. Restart procesu unieważnia niewykorzystane bilety.

Żądania zmieniające stan wymagają `X-Requested-With: app`. API kontroluje `Origin`, limituje JSON do 8 KiB i zwraca `Cache-Control: no-store`. Nie loguje haseł, tokenów, adresów e-mail ani parametrów SQL. Adres IP sesji jest przechowywany jako HMAC z kluczem serwera. `X-Forwarded-For` jest ignorowany, dopóki adres pośrednika nie jest jawnie zaufany.

Limity w jednym procesie:

- Logowanie: 5 prób na konto w 15 min (wspólny limit loginu i e-maila), 20 prób na IP. Udane logowanie zeruje limit konta. Nieudane próby mają opóźnienie 100–800 ms; przekroczenie limitu daje 429 i `Retry-After`.
- Rejestracja: 5 prób na IP na godzinę; poza loopback obowiązkowo Turnstile, sprawdzany na serwerze z kontrolą `hostname` i `action=register`.
- Żądania wiadomości: 10 na IP i 3 na adres na godzinę; potwierdzenia/reset mają osobne limity.
- Rotacja: 5/min na sesję; bilety: 30/min na sesję. Pamięć limitera i biletów ma ograniczoną pojemność.

## Poczta i uruchamianie

Lokalnie `run.bat` oraz adres `http://localhost:5173/#/account`. Domyślna konfiguracja lokalna zapisuje wiadomości w `target/mail/*.json`; link jest w polu `body`. Nie wysyła rzeczywistych e-maili. Fragment adresu zawiera token; ekran konta usuwa go z widocznego adresu i bieżącego wpisu historii przed żądaniem API. Potwierdzenie wymaga przycisku użytkownika.

Poza loopback potrzebne są HTTPS, `PUBLIC_ORIGIN`, `MAIL_MODE=smtp`, komplet `SMTP_*`, 32-bajtowy `WS_TICKET_KEY` zapisany jako hex oraz klucze Turnstile. SMTP korzysta z uwierzytelniania i wymaganego STARTTLS. Konfiguracja jest w `.env.example` i [CONTRIBUTING.md](../CONTRIBUTING.md). Istniejący `.env` nie jest nadpisywany; serwer nasłuchujący wyłącznie na loopback ma lokalny fallback adresu klienta i poczty.

Ponowne wysłanie linku i reset odpowiadają przed odczytem konta z bazy, aby czas SMTP nie ujawniał istnienia adresu. Wysyłka działa w maksymalnie 4 zadaniach, pod blokadą konta i w transakcji tokenu. Błąd wycofuje token; użytkownik może ponowić żądanie. Nie ma jeszcze trwałej kolejki z automatycznymi ponowieniami po restarcie. Przy rejestracji API czeka na dostarczenie wiadomości i zatwierdzenie konta.

Klient Android wymaga `VITE_API_ORIGIN=https://…` podczas budowania. Adres zostaje dopisany do CSP. Token klienta jest obecnie tylko w RAM; rotacja następuje przed kolejnym żądaniem po 7 dniach. Trwałe logowanie przez Android Keystore i pełny cykl życia powłoki należą do F12A. Backend obu ścieżek jest sprawdzany integracyjnie; w tym kroku nie przebudowywano APK ani nie konfigurowano produkcyjnego dostawcy SMTP/Turnstile.

## Weryfikacja

Końcowy wynik lokalny: **19/19 etapów `check.bat` (183 s)**, bez pominiętych etapów. **14/14 testów kont + 10/10 testów bazy**, **20/20 scenariuszy Playwright**, **10/10 testów Vitest**. Lighthouse mobile: **99 Performance, 100 Accessibility, 100 Best Practices, 100 SEO**. Przeszły też clippy, testy Rust/WASM, balans, typy, Protobuf, metadane SQLx i start serwera z PostgreSQL. Pakiet offline bez osobno ładowanych ilustracji: 198,3 KiB Brotli przy limicie 200 KiB. Formularz rejestracji sprawdzono wizualnie w emulacji telefonu.

14 testów kont na osobnych bazach PostgreSQL 18.6 obejmuje oba logowania, oba rodzaje wylogowania, utrwalenie skrótów, sesje wygasłe i zbanowane, rotację i bilet jednorazowy pod współbieżnością, reset wszystkich sesji, jednorazowość i wygasanie tokenów e-mail, ponowne wysłanie, wszystkie sześć zestawów startowych, rollback po błędzie poczty, uprawnienia do listy sesji, cookies, CORS/CSRF, wersję aplikacji oraz limity konta/IP bez zaufania do podszytego `X-Forwarded-For`.

Testy przeglądarkowe działają na komputerze i w emulacji telefonu. Tryb gościa rozgrywa pełny trening offline; test porównuje pamięć trwałą i ciasteczka, sprawdza zero API i utratę stanu po odświeżeniu. Formularze używają kontrolowanych odpowiedzi API; prawdziwy backend sprawdzają testy Rust/PostgreSQL. Test linku e-mail potwierdza usunięcie tokenu z adresu i brak automatycznego zużycia. Testy jednostkowe klienta dodatkowo sprawdzają, że spóźniony profil nie zastępuje gościa, a dane logowania nie trafiają do browser storage.

Pełny `check.bat` i CI uruchamiają obie grupy PostgreSQL przez `cargo test -p server --tests --locked -- --ignored` oraz kontrolują metadane SQLx. Testy wykorzystują przechwytywanie wiadomości, bez zewnętrznego SMTP i bez wysyłania poczty do prawdziwych odbiorców. Zdalny workflow CI nie był uruchamiany z tej sesji.

## Dalej

F6.3: połączenie WebSocket, `Hello`/`Welcome`, heartbeat i resume. Lobby i czat pozostają w F6.4/F6.5. Ograniczenia niezweryfikowanego konta z R-KNT-02 będą egzekwowane przez moduły rankingu, rozwoju i ekonomii w ich fazach; lokalny gość nie ma do tych operacji dostępu.
