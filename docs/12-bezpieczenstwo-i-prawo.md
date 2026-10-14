# 12. Bezpieczeństwo, fair play i kwestie prawne

> Rozdziały 7–9 opisują wymagania prawne w zakresie potrzebnym do planowania. **To nie jest porada prawna.** Przed publicznym startem dokumenty i decyzje trzeba skonsultować z prawnikiem (krok F13.5 planu).

## 1. Model zagrożeń

| Zasób | Zagrożenie | Zabezpieczenie |
|---|---|---|
| Konta | przejęcie (hasła z wycieków, zgadywanie) | Argon2id, limity prób logowania z opóźnieniami, jednorazowe tokeny resetu (30 min), unieważnienie sesji po zmianie hasła, 2FA (TOTP) obowiązkowe dla administracji |
| Sesje | kradzież identyfikatora | ciasteczko `HttpOnly; Secure; SameSite=Lax`, rotacja przy logowaniu, lista aktywnych sesji z możliwością wylogowania |
| Rozgrywka | zmodyfikowany klient | serwer jest arbitrem; każdy ruch walidowany przez silnik po stronie serwera (R-RUN-06) |
| Ukryte informacje | wyciek ręki, talii lub ziarna | filtrowanie przez `view()` i widoczność zdarzeń; testy kontraktowe, że wiadomości do gracza nie zawierają cudzej ręki ani ziarna |
| Losowość | przewidzenie tasowania lub nagród | ziarno 256 bitów z CSPRNG, ChaCha8; zobowiązanie do ziarna ujawnianego po walce (ADR-007); losowanie nagród po stronie serwera |
| Ekonomia | duplikacja przez wyścigi, powtórzone żądania | transakcje, blokady wierszy, `CHECK`, klucze idempotencji, testy współbieżności, uzgadnianie księgi (`08-serwer.md`, rozdz. 6) |
| Uczciwość rankingu | „podkładanie się”, farmienie, wiele kont | wykrywanie i sankcje (rozdz. 4) |
| Społeczność | spam, obraźliwy czat | limity, filtry, wyciszenia, zgłoszenia, dziennik moderacji (rozdz. 5) |
| Dostępność usługi | DDoS, zalewanie wiadomościami | Cloudflare, limity połączeń, wiadomości i rozmiaru (`07-protokol-sieciowy.md`, rozdz. 6), limity czasu |
| Aplikacja webowa | XSS, CSRF, CSWSH, wstrzyknięcia SQL | CSP, escapowanie, SameSite + nagłówek `X-Requested-With`, kontrola `Origin` przy WebSocket, zapytania parametryzowane (`sqlx`) |
| Łańcuch dostaw | złośliwa lub dziurawa zależność | pliki blokad, `cargo-deny`, `cargo-audit`, `pnpm audit`, Renovate, akcje GitHub przypięte do SHA, minimalne uprawnienia tokenów CI |
| Serwer | włamanie | SSH tylko kluczami i z listy adresów, zapora (ruch HTTP tylko z adresów Cloudflare), automatyczne poprawki bezpieczeństwa, kontenery bez roota, system plików tylko do odczytu |
| Kopie zapasowe | wyciek danych z kopii | szyfrowanie kopii po stronie klienta (WAL-G / pgBackRest), osobne klucze dostępu tylko do zapisu |
| Panel administracyjny | nadużycie uprawnień | 2FA, lista adresów IP, osobny pakiet, pełny dziennik audytu |
| Token aplikacji Android | kradzież z telefonu (np. z kopii zapasowej lub na urządzeniu z rootem) | szyfrowanie kluczem z Android Keystore, wyłączenie tokenu z kopii zapasowych Androida, wymiana co 7 dni, unieważnianie z listy sesji (ADR-015) |
| Bilet WebSocket | przechwycenie i ponowne użycie | ważność 30 s, jednorazowość, maskowanie w logach |
| Aktualizacje paczek web w aplikacji | podmieniona paczka | podpis Ed25519 weryfikowany kluczem wbudowanym w APK, HTTPS, wycofanie po nieudanym starcie (`19-aplikacja-android.md`, rozdz. 5) |
| Klucz podpisu aplikacji Android | kradzież (fałszywe aktualizacje) albo utrata (brak aktualizacji APK) | kopie offline w dwóch miejscach, dostęp w CI tylko w chronionym środowisku z ręcznym zatwierdzeniem, osobny klucz przesyłania do Google Play |

## 2. Konta i uwierzytelnianie

- Hasła: minimum 10 znaków, sprawdzenie na liście popularnych haseł, brak wymogów „wielka litera + cyfra”. Hash Argon2id z parametrami OWASP (ADR-010).
- Logowanie: 5 nieudanych prób na konto w 15 min, potem rosnące opóźnienie; 20 na adres IP. Komunikat błędu nie zdradza, czy konto istnieje.
- Rejestracja: CAPTCHA przyjazna prywatności (np. Cloudflare Turnstile) i limit kont na adres IP w czasie.
- Potwierdzenie e-maila zamienia konto gościa w pełne (R-KNT-02). Zmiana e-maila wymaga potwierdzenia na nowy adres i powiadomienia na stary.
- Opcjonalnie później: 2FA dla graczy, passkeys (WebAuthn), logowanie zewnętrzne.

## 3. Integralność rozgrywki

- Klient jest niezaufany. Wszystkie decyzje (ruchy, koszty, wyniki, nagrody) podejmuje serwer.
- Ruch spoza zasad kończy się błędem `INVALID_MOVE`. Seria takich błędów od jednego klienta to sygnał manipulacji (rozdz. 4).
- Limity czasu liczy serwer. Klient nie może „zatrzymać zegara”.
- Zobowiązanie do ziarna pozwala graczom sprawdzić po walce, że tasowanie nie było zmieniane w trakcie (ADR-007). Strona powtórki pokazuje ziarno i jego skrót.

## 4. Fair play i wykrywanie nadużyć

Zasada R-FAI-01: zakaz umawiania wyników i nabijania PD lub rankingu.

**Sygnały** (zadanie okresowe co 15 min, `08-serwer.md`, rozdz. 11):

| Sygnał | Przykład |
|---|---|
| Częste walki tej samej pary | ponad 10 walk w 24 h między tymi samymi kontami |
| Naprzemienne wyniki | wygrane na zmianę, zwłaszcza przy jednokierunkowym przepływie rankingu |
| Walki bez gry | szybka ucieczka, zero zmian ustawień i zero czarów przez całą walkę |
| Wspólne urządzenie lub sieć | ten sam skrót IP lub odcisk urządzenia u kont, które grają ze sobą lub się wymieniają |
| Jednostronne wymiany | nowe konta przekazujące karty i Dukaty jednemu odbiorcy |
| Automatyzacja | nienaturalnie stałe czasy ruchów przez wiele walk |
| Manipulacja klientem | powtarzające się `INVALID_MOVE` i nietypowe wiadomości |

**Postępowanie:**
1. Sygnały mają wagi, a suma powyżej progu trafia do kolejki moderacji. Automatycznie blokowane są tylko przypadki oczywiste (np. zautomatyzowana rejestracja).
2. Moderator ogląda historię i powtórki walk w panelu.
3. Sankcje rosnące z każdym wykroczeniem (jak w oryginale, S3): ostrzeżenie, cofnięcie PD i rankingu zdobytych w spornych walkach, blokada czasowa, blokada stała.
4. Każda decyzja trafia do `audit_log`, a gracz dostaje uzasadnienie.

**Wiele kont:** regulamin dopuszcza jedno konto na osobę. Samo posiadanie kilku kont nie jest automatycznie karane, karane jest wykorzystywanie ich do nadużyć.

## 5. Moderacja społeczności

- Czat: limity częstotliwości, blokada powtórzeń, filtr słów (konfigurowalny, PL i EN), maksymalna długość. Linki nie są klikalne.
- Zgłoszenia od graczy (wiadomość, gracz, nazwa klanu) z kontekstem: ostatnie wiadomości z kanału.
- Wyciszenie czasowe, blokada, zmiana obraźliwej nazwy klanu lub opisu.
- Dziennik czatu przechowywany 30 dni, tylko na potrzeby moderacji.
- Gracz może wyłączyć czat ogólny i używać tylko prywatnego oraz klanowego.

## 6. Bezpieczeństwo aplikacji i infrastruktury

**Nagłówki HTTP:** `Content-Security-Policy` (bez `unsafe-inline` i `unsafe-eval`; `wasm-unsafe-eval` dla silnika; `frame-ancestors 'none'`), `Strict-Transport-Security` (z `preload`), `X-Content-Type-Options: nosniff`, `Referrer-Policy: strict-origin-when-cross-origin`, `Permissions-Policy` wyłączające nieużywane API.

**Serwer:**
- Wszystkie wejścia walidowane (typy z Protobuf, długości, zakresy).
- Błędy bez szczegółów wewnętrznych w odpowiedziach. Logi bez danych osobowych, adresy IP tylko jako skróty z solą.
- Brak CORS (wszystko z jednej domeny).

**Zależności:** `cargo-deny` (licencje, znane podatności, zduplikowane wersje) i `pnpm audit` w CI. Aktualizacje przez Renovate co tydzień, poprawki bezpieczeństwa od razu.

**Kontenery:** obraz serwera na bazie distroless lub `scratch` (statyczna binarka), użytkownik bez uprawnień roota, system plików tylko do odczytu, ograniczone możliwości jądra (`cap_drop: ALL`).

**Sekrety:** poza repozytorium; w CI jako sekrety GitHub, na serwerze w pliku środowiskowym z prawami 600 lub przez Docker secrets. Rotacja haseł bazy i kluczy co najmniej raz w roku i po każdym incydencie.

**Plik `/.well-known/security.txt`** z kontaktem do zgłaszania podatności.

## 7. Prywatność i RODO

| Temat | Ustalenie |
|---|---|
| Administrator danych | firma właściciela projektu (decyzja właściciela; dane rejestrowe do uzupełnienia w polityce prywatności). Ta sama firma jest wydawcą aplikacji w Google Play (`19-aplikacja-android.md`, rozdz. 7) |
| Zbierane dane | login, e-mail, hash hasła, skrót IP i user agent (bezpieczeństwo), dane gry, czat, poczta w grze, zgłoszenia |
| Czego nie zbieramy | imię i nazwisko, data urodzenia, adres, dane płatnicze (brak płatności) |
| Podstawy prawne | wykonanie umowy (świadczenie usługi gry); prawnie uzasadniony interes (bezpieczeństwo, wykrywanie nadużyć, moderacja); zgoda dla opcjonalnej analityki i komunikacji marketingowej |
| Retencja | czat 30 dni; logi serwera 14–30 dni; sesje do wygaśnięcia; konta usunięte: anonimizacja po 30 dniach karencji (historia walk zostaje z anonimowym identyfikatorem) |
| Prawa osób | eksport danych (`GET /api/me/export`, JSON), zmiana danych, usunięcie konta (`DELETE /api/me`), sprzeciw; kontakt e-mailowy w polityce prywatności |
| Podmioty przetwarzające | hosting, Cloudflare, dostawca e-mail, ewentualnie zewnętrzny monitoring. Z każdym umowa powierzenia (DPA); preferowany monitoring błędów hostowany samodzielnie (GlitchTip), żeby uniknąć transferu danych |
| Ciasteczka | tylko niezbędne (sesja), więc bez banera zgody. Analityka bez ciasteczek (np. Plausible lub Umami hostowane samodzielnie) albo za zgodą |
| Wiek | regulamin: ukończone 16 lat albo zgoda opiekuna. Nie zbieramy daty urodzenia, tylko oświadczenie |
| Naruszenia | procedura zgłoszenia do Prezesa UODO w ciągu 72 godzin (rozdz. 10) |
| Dokumentacja | polityka prywatności, regulamin, rejestr czynności przetwarzania |
| Aplikacja Android | formularz „Bezpieczeństwo danych” w Google Play zgodny z polityką prywatności; minimalne uprawnienia; usuwanie konta w aplikacji i przez stronę WWW (`19-aplikacja-android.md`, rozdz. 12) |

## 8. Własność intelektualna

**Co prawdopodobnie jest chronione w oryginale:** logo, grafiki, teksty kart i opisy postaci, elementy fabuły (świat, „Wybraniec”, opisy szkół) oraz charakterystyczne nazwy postaci. Nazwa „Cellmons” nie ma obecnie żywego znaku towarowego ani w TMview, ani w USPTO (sprawdzenie z 13.10.2026, `20-nazwa-i-znak-towarowy.md`, rozdz. 2). Może ją jednak chronić np. prawo do tytułu lub do oznaczenia przedsiębiorstwa. Tej nazwy i tak nie używamy (ADR-012).

**Co co do zasady nie jest chronione:** zasady i mechanika gry, ogólne pojęcia (atak, obrona, magia, nekromancja, combo), układ rund i liczby jako takie. Skopiowanie w całości zestawienia danych (np. pełnej listy kart z parametrami) mogłoby jednak naruszać prawo do bazy danych, dlatego dane referencyjne służą tylko do projektowania i nie trafiają do produktu (ADR-012).

**Właściciele praw:** Pogram / Toolzz & Toyzzz (lata 2003–2009) oraz nieustalony operator wersji z 2010 r. Ich aktualny status trzeba sprawdzić w KRS. Gdyby kiedyś zapadła decyzja o użyciu oryginalnej nazwy, jedyną bezpieczną drogą jest licencja od właściciela praw.

**Nasza treść:**
- **Grafiki** tworzy asystent AI (Claude) jako kod SVG, na podstawie opisów i decyzji właściciela (ADR-016).
  - Ochrona prawnoautorska utworów powstałych w dużej mierze z udziałem AI jest niepewna: zależy od wkładu twórczego człowieka i od jurysdykcji. Dlatego dokumentujemy wkład właściciela (opis, wybór wariantu, poprawki, kompozycja).
  - Jako wzorów nie używamy cudzych chronionych grafik, w tym grafik z oryginalnego Cellmons.
  - Rozpoznawalność chroni znak towarowy, bo ochrona marki nie zależy od praw autorskich do grafik.
- **Rejestr pochodzenia** każdej grafiki i tekstu: autor lub narzędzie, data, opis zamówienia, zatwierdzający, licencja. Gdyby kiedyś grafiki zamawiano u ilustratorów: umowy z przeniesieniem autorskich praw majątkowych.
- **Nazwa produktu:** „Spellgambit: Arcane Class”. Firma zgłasza znak słowny SPELLGAMBIT w EUIPO w kl. 9 i 41. Wyniki sprawdzenia znaków i domen, wykaz towarów i usług oraz kroki zgłoszenia są w `20-nazwa-i-znak-towarowy.md`.

## 9. Pozostałe regulacje

- **Losowe zestawy kart:** kupowane wyłącznie za walutę zdobywaną w grze (ADR-011) i z jawnymi szansami (R-SKL-04). Gdyby w przyszłości pojawiły się płatności, należy unikać sprzedaży losowych zawartości za pieniądze i sprawdzić aktualne przepisy unijne i krajowe dotyczące mechanizmów losowych oraz ochrony konsumentów w grach.
- **Handel walutą i kartami za prawdziwe pieniądze** jest zakazany regulaminem.
- **Akt o usługach cyfrowych (DSA):** czat, opisy i nazwy klanów to treści użytkowników. Potrzebne są: punkt kontaktowy, mechanizm zgłaszania nielegalnych treści i uzasadnianie decyzji moderacyjnych. Zakres obowiązków zależy od wielkości usługodawcy, więc trzeba go potwierdzić przed startem.
- **Dostępność:** darmowa gra bez sprzedaży raczej nie podlega obowiązkom Europejskiego aktu o dostępności, ale WCAG 2.2 AA i tak jest celem projektu (`10-klient-web.md`, rozdz. 12).

## 10. Reagowanie na incydenty

1. **Wykrycie:** alert z monitoringu, zgłoszenie gracza albo `security.txt`.
2. **Ocena:** zakres, dotknięte dane i konta, czy incydent trwa.
3. **Ograniczenie:** tryb konserwacji, wyłączenie funkcji flagą (np. `trading`), unieważnienie sesji, rotacja sekretów.
4. **Usunięcie przyczyny** i wdrożenie poprawki. Przywrócenie danych z kopii, jeśli trzeba (PITR).
5. **Komunikacja:** ogłoszenie dla graczy. Jeśli naruszono dane osobowe: zgłoszenie do Prezesa UODO w 72 h i, gdy ryzyko jest wysokie, powiadomienie osób, których dotyczy.
6. **Analiza po incydencie** (bez szukania winnych): przyczyna, oś czasu, działania zapobiegawcze, aktualizacja procedur w `15-infrastruktura-i-wdrozenie.md`.
