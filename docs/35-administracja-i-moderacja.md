# F12: administracja, moderacja i fair play

Stan implementacji: 2026-10-16. Raport dotyczy lokalnego środowiska i testów na osobnych bazach PostgreSQL. Nie nadano uprawnień żadnemu rzeczywistemu kontu właściciela. Raport z danych ludzkiej bety pozostaje do wykonania po jej przeprowadzeniu.

## Dostęp do panelu

Panel `/#/admin` jest ładowany dopiero po wejściu na tę stronę. Odnośnik jest na stronie konta moderatora lub administratora. Serwer niezależnie sprawdza:

1. Skonfigurowanie `ADMIN_TOTP_KEY` i `ADMIN_ALLOWED_IPS`; brak obu wyłącza panel, a podanie tylko jednej wartości przerywa start serwera.
2. Faktyczny adres połączenia z uwzględnieniem wyłącznie jawnie zaufanych proxy. Nagłówek `X-Forwarded-For` od niezaufanego klienta nie daje dostępu. Lista może mieć do 64 sieci; `/0` jest zabronione.
3. Aktualną, niewygasłą sesję, aktywne konto, potwierdzony e-mail i bieżącą rolę `moderator` lub `admin`.
4. Dodatkowy dostęp na 15 minut, związany z sesją, kontem, skrótem adresu i wersją drugiego składnika.

Konfiguracja wymaga hasła i potwierdzenia kodu. Oczekujący sekret wygasa po 10 minutach; ponowienie konfiguracji przed wygaśnięciem oddaje ten sam sekret. Odblokowanie wymaga hasła i kodu. Sekret jest przechowywany jako AES-256-GCM z losowym nonce i identyfikatorem konta w danych uwierzytelnianych. Zmiana konta w wierszu lub szyfrogramu powoduje odrzucenie. Klucz szyfrowania jest wyprowadzany z osobnego klucza administracji, a nie z hasła gracza.

TOTP używa sześciu cyfr, kroku 30 s i tolerancji jednego kroku. Serwer zapisuje ostatni przyjęty krok i odrzuca jego ponowne użycie przez inne żądanie. Ponowienie tego samego zakończonego żądania jest idempotentne i nie przedłuża dostępu. Wymóg jednokrotnego użycia pochodzi z [RFC 6238](https://datatracker.ietf.org/doc/html/rfc6238); implementacja korzysta z [totp-rs 6](https://docs.rs/totp-rs/6.0.0/totp_rs/struct.Builder.html) i [aes-gcm](https://docs.rs/aes-gcm/0.11.1/aes_gcm/).

Hasła, kody, sekrety konfiguracji i tokeny nie trafiają do dziennika decyzji ani pamięci trwałej przeglądarki. Znikają z formularza po potwierdzeniu lub opuszczeniu panelu. API ma `Cache-Control: no-store`, istniejące zabezpieczenie CSRF/CORS, limit treści 8 KiB oraz osobne limity prób drugiego składnika i operacji.

## Uprawnienia i decyzje

| Operacja | Moderator | Administrator |
|---|---|---|
| Przegląd graczy, zgłoszeń, czatu, klanów, sygnałów i audytu | tak | tak |
| Ostrzeżenie, blokada, wyciszenie zwykłego gracza lub mentora | tak | tak |
| Maksymalna blokada | 7 dni | 365 dni lub bezterminowo |
| Maksymalne wyciszenie | 72 h | 365 dni |
| Edycja tekstów klanu i rozpatrzenie zgłoszenia | tak | tak |
| Filtry słów, ogłoszenia, konserwacja, turnieje systemowe | nie | tak |
| Nadanie roli gracza, mentora lub moderatora | nie | tak, potwierdzony e-mail celu |
| Zmiana konta administratora lub nadanie roli admina | nie | wyłącznie konsola właściciela |

Każda decyzja wymaga powodu 5–2000 znaków. Transakcja blokuje użytkowników w kolejności UUID, ponownie sprawdza uprawnienia i drugi składnik, wykonuje zmianę oraz zapisuje identyfikator żądania i niezmienny audyt. Ten sam identyfikator z innymi danymi jest konfliktem. Ponowienie starej blokady po jej cofnięciu nie nakłada jej ponownie.

Audyt przechowuje powód i operację oraz poprzedni stan zmienianych ustawień. Baza odrzuca `UPDATE` i `DELETE` audytu. Nie jest to deklaracja odporności na właściciela bazy z uprawnieniami DDL.

Blokada rozłącza WebSocket i wyłącza pozostałe operacje gry. Wygaszenie blokady jest sprawdzane przy uwierzytelnianiu, bez dodatkowego oczekiwania na zadanie godzinowe. Istniejące, niewygasłe poświadczenie zablokowanego użytkownika może odczytać wyłącznie jego powiadomienia przez `/api/notices`; nie daje dostępu do gry. Wyciszenie obejmuje czat publiczny, prywatny, klanowy, wyprawy i pocztę.

## Zgłoszenia i komunikacja

Przy wiadomości czatu, profilu gracza i profilu klanu jest przycisk zgłoszenia. Klient wysyła identyfikator obiektu i powód; treść dowodowa pochodzi z bazy serwera. Osoba postronna nie może zgłosić ani odczytać kontekstu cudzej wiadomości prywatnej. Dostęp do kontekstu klanowego wymaga członkostwa, a do pokoju wyprawy uczestnictwa w czasie wiadomości. Dowód jest zachowany w zgłoszeniu niezależnie od 30-dniowej retencji dziennika czatu.

Zgłoszenia mają limit 20 dziennie na konto oraz limit wejściowy API. Odpowiedź nie oddaje prywatnego kontekstu. Rozpatrzenie lub odrzucenie powiadamia zgłaszającego. Powody ostrzeżeń, blokad i wyciszeń są dostępne na stronie konta. Ogłoszenia i komunikat konserwacji są odczytywane przez zalogowane lobby; gość nie wywołuje tych API.

Filtry stosują NFKC, małe litery, usunięcie wybranych niewidocznych znaków formatowania i granice całych słów. Nie uruchamiają wyrażeń regularnych podanych przez administratora. Przykładowo filtr `ass` nie blokuje `classical`. To moderacja prostych fraz, nie ocena kontekstu językowego ani kompletne wykrywanie omijania filtrów. Mechanizm normalizacji opisuje [dokumentacja unicode-normalization](https://docs.rs/unicode-normalization/0.1.25/unicode_normalization/trait.UnicodeNormalization.html).

## Turnieje systemowe

Tworzy je tylko administrator z drugim składnikiem i dozwolonego IP. Są bezpłatne. Zamrożone warunki obejmują zwykłe ograniczenia poziomu, reinkarnacji i składu klanowego oraz jedną nagrodę:

- Dukaty: 1–1 000 000 w puli, zasilone przez źródło systemowe w tej samej transakcji. Zwycięska drużyna dzieli je równomiernie; resztę przypisuje kolejność UUID.
- Postać: jeden zwykły szablon; bez cieni, wysypiskowych postaci i osobnych dróg reinkarnacji.
- Karta: jedna karta z kanonicznego katalogu.
- Zestaw: zamrożone pule sklepu i wersja treści; prywatne ziarno otwierania zostaje zapisane wraz z nieotwartym zestawem.

Każdy członek zwycięskiej drużyny otrzymuje nagrodę rzeczową. Posiadana już postać nie daje kopii ani zamiennika; liczba kart ma istniejący limit całkowitoliczbowy. Te zasady są podane przed zapisem. Rozliczenie zachodzi raz, w transakcji kończącej turniej. Odwołanie lub wygaśnięcie spala niewykorzystaną pulę systemową, a opłaty graczy zwraca jak dotychczas. Systemowy zestaw ma cenę 0 i jawne źródło `tournament`; nie udaje zakupu.

Zwykłe API tworzenia nie przyjmuje pola nagrody ani rodzaju systemowego. Odwołanie systemowego turnieju przed startem wymaga panelu. Trwające walki używają tego samego trwałego harmonogramu i rozliczenia co turnieje graczy.

## Mentorzy i sygnały nadużyć

Rola mentora ma oznaczenie w lobby i profilu, ale nie daje uprawnień panelu. Przy zwycięstwie mentora nie ma PD ani transferu rankingu. Wygrana nad mentorem jest rozliczana zwyczajnie. Dla tej zasady uwzględniana jest rola zamrożona na początku walki oraz aktualna rola przy rozliczeniu, aby zmiana w trakcie walki nie obchodziła ograniczenia. Nagrody turniejowe pozostają osobną, jawną zasadą.

Polecenie `server abuse-report` tworzy dzienne sygnały do ręcznego przeglądu: powtarzane krótkie i jednostronne walki arenowe, co najmniej cztery konta ze wspólnym skrótem sieci, wielokrotne jednostronne wymiany i nietypowo duże wpływy Dukatów. Nie zwraca adresów IP ani ich skrótów w dowodach. Wspólna sieć, prezenty i wysokie nagrody mogą być legalne. Nie ma automatycznych blokad. Powtórne skanowanie zastępuje wynik, zamiast dodawać licznik; niezmieniony, już sprawdzony sygnał pozostaje sprawdzony. Dla par walk kolejność graczy jest normalizowana.

Dane testowe dowodzą działania transakcji i powtarzalności. Nie zastępują raportu z bety i nie dowodzą skuteczności wykrywania rzeczywistych oszustw.

## Uruchomienie i odzyskiwanie dostępu

Wprowadź oba ustawienia administracji z `.env.example`, używając osobnego losowego klucza. Z konsoli z dostępem do bazy, po sprawdzeniu tożsamości operatora:

```text
server admin role LOGIN admin "Powód nadania uprawnień"
server admin role LOGIN moderator "Powód nadania uprawnień"
server admin reset-factor LOGIN confirm "Powód resetu drugiego składnika"
server abuse-report
```

Konsola wymaga aktywnego konta z potwierdzonym e-mailem i zapisuje audyt bez udawania innego użytkownika. Nie pozwala zdegradować ostatniego aktywnego administratora. Reset usuwa czynnik i wszystkie dodatkowe dostępy tego konta; kolejna konfiguracja wymaga hasła. Nie ma publicznego endpointu nadania pierwszego administratora ani wyłączenia 2FA. Utrata klucza wymaga kontrolowanego resetu czynników i ponownej konfiguracji; klucz powinien mieć kopię w menedżerze sekretów.

## Weryfikacja

Źródła testów: `crates/server/tests/admin.rs`, `crates/server/tests/admin/control.rs`, `e2e/admin.ts` oraz istniejące testy turniejów i bramki. Dodatkowe testy jednostkowe są w modułach szyfrowania, konfiguracji i normalizacji.

Testy sprawdzają między innymi: wyłączony panel, fałszowanie IP, brak roli mentora w panelu, hasło i TOTP, szyfrowanie przypisane do konta, wygaśnięcie i zmiany sesji/roli, ponowienia sieciowe, 100 równoczesnych decyzji i potwierdzeń, prywatność zgłoszeń, limity moderatora, konserwację, cztery nagrody systemowe i uzgodnienie księgi, oznaczenia mentorów oraz sygnały bez automatycznych sankcji.

Pełny przebieg `scripts/check.ps1`: **19/19 kroków poprawnych, 419 s** (`target/f12-complete-check.log`). Zawiera **96 testów PostgreSQL**, w tym 10 administracji, testy natywne i WASM, 25 testów Vitest oraz 24 Playwright. Lighthouse mobile: **100/100/100/100**. Oddzielny przebieg przepływu lobby i administracji: desktop 44,4 s, emulacja Pixel 5 46,8 s. Test obejmuje rzeczywistą konfigurację TOTP, zgłoszenie wiadomości, rozpatrzenie, wyciszenie, utratę odpowiedzi po zapisie i ponowienie tego samego żądania, cofnięcie wyciszenia, powiadomienia, oznaczenie mentora i zablokowanie panelu.

Przegląd i testy naprawiły m.in. wyścig aktywnego harmonogramu z ręcznym testem nagrody, utratę filtra podczas przełączania zakładek, możliwość ponownego wykonania starej decyzji, brak informacji o nagrodzie przed zapisem i brak wyłączenia PD/rankingu przy zwycięstwie mentora. Testy nie obejmują rzeczywistych danych z bety ani zewnętrznego audytu penetracyjnego.

Podział katalogu kart na moduł ładowany z kolekcją/walką zachowuje pełny tryb offline. Kontrola budżetów liczy dodatkowo całe początkowe wejście do lobby, łącznie ze wspólnym kodem startowym; limit pozostaje 70 KiB. Kod panelu, zgłoszeń i powiadomień ma osobny limit 20 KiB i jest wykluczony z pakietu offline. Limit pełnego pakietu offline pozostaje 200 KiB.
