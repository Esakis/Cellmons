# 17. Ryzyka i otwarte pytania

## 1. Rejestr ryzyk

Skala: P = prawdopodobieństwo, W = wpływ (N niski, Ś średni, W wysoki). Rejestr przegląda się przy każdym kamieniu milowym.

| ID | Ryzyko | P | W | Ograniczenie ryzyka | Kiedy sprawdzić |
|---|---|---|---|---|---|
| RY-01 | Naruszenie praw do oryginału (nazwa, teksty, grafiki, fabuła) | Ś | W | własna treść (ADR-012), dane referencyjne poza produktem, przegląd prawny przed startem | F3, F13.5 |
| RY-02 | Brak tekstów większości kart utrudnia odtworzenie „ducha” gry | W | Ś | projektowanie efektów według rodzin i archetypów, symulacje, testy z graczami w becie | F3, F10 |
| RY-03 | Problemy z balansem (przewaga inicjatywy, dominujące karty lub postacie) | Ś | W | symulator i progi w CI, telemetria, szybkie wydania treści, kompensata inicjatywy w razie potrzeby | F3, F10, F15 |
| RY-04 | Zakres (pełna gra sieciowa z ekonomią i turniejami) za duży dla jednej osoby | W | W | kamienie milowe z grywalną wersją co kilka tygodni, flagi funkcji, lista cięć (rozdz. 3) | każdy kamień milowy |
| RY-05 | Puste lobby na starcie (za mało graczy na PvP) | W | W | boty w sali treningowej, trening offline, wyprawy PvE, turnieje systemowe z botami do uzupełnienia, beta na zaproszenia, budowanie społeczności przed startem | M4, M6 |
| RY-06 | Ekonomia bez płatności wymknie się spod kontroli (inflacja lub niedobór Dukatów) | Ś | Ś | metryki źródeł i ujść, parametry `economy.*`, przeglądy cotygodniowe | F9, F15 |
| RY-07 | Oszustwa („podkładanie się”, wiele kont, boty) | Ś | Ś | wykrywanie i sankcje (`12-bezpieczenstwo-i-prawo.md`, rozdz. 4) | F12, F15 |
| RY-08 | Niespójna lub zbyt prosta grafika (SVG asystenta AI, ADR-016; od ADR-021 obrazy różnych malarzy) | Ś | Ś | wspólny kadr i korekta barwna obrazów, ikony z jednego zestawu, przegląd i zatwierdzanie przez właściciela, ograniczona i spójna paleta | F3, F9, F10 |
| RY-09 | Wolniejszy rozwój w Rust (krzywa uczenia, czas kompilacji) | Ś | Ś | mały, dobrze wydzielony silnik; szablony modułów serwera; ponowne rozważenie ADR-001/002 po F1, jeśli tempo jest wyraźnie za niskie | po F1 |
| RY-10 | WASM lub animacje za wolne na słabych telefonach | N | Ś | F5: WASM 68 KiB Brotli, Lighthouse 100 i trening offline potwierdzone w emulatorze; pierwszy start aktywności 3,4 s, kolejne 1,5/1,1 s. Brak pomiarów 60 FPS i gotowości UI na fizycznych telefonach — wymagane 2–3 urządzenia przed zamknięciem F5.6; szczegóły w `21-trening-offline-testy.md` | F5.6, F13 |
| RY-11 | Jeden serwer to pojedynczy punkt awarii | Ś | W | kopie z PITR, monitoring na osobnej maszynie, procedura R4 (nowy serwer ≤ 1 h) | F14 |
| RY-12 | Sieci szkolne i firmowe blokują WebSocket | N | Ś | WSS na porcie 443 przez Cloudflare; pomiar odsetka nieudanych połączeń w becie | M4 |
| RY-13 | Wymogi prawne (RODO, DSA) niespełnione przy starcie | Ś | W | dokumenty i mechanizmy w F13.5, konsultacja prawna | F13 |
| RY-14 | Zależność od Cloudflare | N | Ś | możliwość przejścia na bezpośredni TLS w Caddy (ACME) i innego dostawcę DNS w ciągu godzin | F14 |
| RY-15 | Zbyt długa progresja zniechęci graczy (np. 500 wygranych do poz. 20) | Ś | Ś | kalibracja Z-XP symulacjami i telemetrią, codzienne nagrody | F10, F15 |
| RY-16 | Odrzucenie lub opóźnienie publikacji w Google Play (zasady wobec aplikacji-opakowań stron, weryfikacja konta organizacji i D-U-N-S, recenzje) | Ś | W | pełna gra offline i funkcje natywne w aplikacji, opis w sklepie pokazujący rozgrywkę, konto organizacji zakładane z wyprzedzeniem (F9.6), test zamknięty przed produkcją | F9, F12A |
| RY-17 | Słaba wydajność lub błędy WebView na starszych telefonach | Ś | Ś | `minSdk` 26, minimalna wersja WebView z komunikatem o aktualizacji, test na telefonach już w F5.6, macierz urządzeń | F5, F12A |
| RY-18 | Utrata lub wyciek klucza podpisu aplikacji | N | W | kopie offline w dwóch miejscach, osobny klucz przesyłania, dostęp w CI tylko w chronionym środowisku, procedura R13 | F12A |
| RY-19 | Android zamyka aplikację w tle i gracz przegrywa walkę przez rozłączenie | Ś | Ś | powiadomienie lokalne z czasem na powrót, natychmiastowe wznawianie sesji, telemetria rozłączeń; ewentualnie usługa pierwszoplanowa (Z-AND-2) | F12A, F15 |
| RY-20 | Sprzeciw właściciela wcześniejszego znaku z „GAMBIT” lub „ARCANE” albo zgłoszenie „SPELLGAMBIT” przez kogoś innego przed nami | N | W | sprawdzenie z 13.10.2026 bez kolizji (`20-nazwa-i-znak-towarowy.md`, rozdz. 2 i 3); znak słowny bez podtytułu; wykaz usług bez edukacji; przegląd rzecznika patentowego; zgłoszenie w EUIPO jak najszybciej (P-13); domeny .pl, .eu i .gg | F0.7, potem przy każdym kamieniu milowym |

## 2. Otwarte pytania do właściciela projektu

Każde pytanie ma odpowiedź domyślną, przyjętą w dokumentacji, dopóki nie zapadnie inna decyzja.

| ID | Pytanie | Domyślnie | Do kiedy |
|---|---|---|---|
| P-01 | Nazwa produktu i własny świat (fabuła, nazwy szkół, postaci, tytułów) | **Nazwa zdecydowana (13.10.2026):** „Spellgambit: Arcane Class”, znak słowny SPELLGAMBIT (`20-nazwa-i-znak-towarowy.md`). Otwarte: własny świat. W kodzie nazwy neutralne | świat: przed betą (F9.6) |
| P-02 | Kto przygotuje grafikę? | **Zdecydowane:** interfejs i ramki to SVG asystenta AI (ADR-016); ilustracje i portrety to obrazy z domeny publicznej, a ikony pochodzą z game-icons.net (ADR-021); zatwierdza właściciel | — |
| P-03 | Rynek i języki | polski i angielski od początku | F5 |
| P-04 | Kto jest administratorem danych osobowych? | **Zdecydowane:** firma właściciela projektu. Do uzupełnienia: dane rejestrowe w polityce prywatności i na koncie Google Play | F13 |
| P-05 | Czy wyrównywać przewagę inicjatywy (np. +1 K dla gracza bez inicjatywy)? | nie, chyba że symulacje pokażą > 60% wygranych | po F3 |
| P-06 | Talia 20 czy 30 kart? (późna wersja klasyczna zapowiadała 30) | 20, parametr `deck.size` | po F10 |
| P-07 | Zakres treści na start publiczny | ok. 130 kart i 16 postaci na betę, pełne ok. 330 kart w wersji kompletnej | M4 |
| P-08 | Czy repozytorium ma być publiczne (open source)? | prywatne; licencja do wyboru przy ewentualnym otwarciu (kod i treść mogą mieć różne licencje) | przed F14 |
| P-09 | Kto moderuje czat i zgłoszenia? | właściciel + zaufani moderatorzy z bety | M4 |
| P-10 | Przyszła monetyzacja | brak (ADR-011); możliwa faza opcjonalna bez losowych płatnych zawartości | po starcie |
| P-11 | Aplikacje mobilne | **Zdecydowane:** Android przez Capacitor, w Google Play i jako APK (ADR-014). Otwarte: wersja iOS (ta sama powłoka; konto Apple Developer i komputer Mac) | iOS po starcie |
| P-12 | Konto Google Play | konto organizacji na firmę (wymaga numeru D-U-N-S); kto je zakłada i kto ma dostęp do kluczy podpisu | F9.6 |
| P-13 | Kiedy zgłosić znak SPELLGAMBIT: od razu (900 € opłat) czy po starcie SME Fund 2027 (zwrot 75%, maks. 700 €)? Edycja 2027 nie jest jeszcze ogłoszona, a bon trzeba dostać przed zgłoszeniem | od razu: w UE znak dostaje pierwszy zgłaszający, a domenę spellgambit.com ktoś zarejestrował 8.10.2026 (`20-nazwa-i-znak-towarowy.md`, rozdz. 4) | przed pierwszym publicznym użyciem nazwy (F0.7) |

## 3. Lista cięć

Gdy harmonogram się wydłuża, te elementy można przesunąć na po starcie bez naruszania rdzenia gry:

1. Turnieje klanowe (R-TUR-08). Zwykłe turnieje zostają.
2. Poczta offline (R-KOM-04). Czat prywatny zostaje.
3. Wysypisko i postacie specjalne (R-WYS).
4. Cienie (R-CIE).
5. Poziom trudności „Męka” w wyprawach.
6. Zapisane zestawy talii (R-TAL-06).
7. Rola mentora (R-FAI-02).
8. Warstwa efektów cząsteczkowych w walce.

**Nie podlegają cięciu:** zasady pojedynku, trening z AI, PvP z rankingiem, rozwój postaci do reinkarnacji, talie i kolekcja, bezpieczeństwo, kopie zapasowe i monitoring.
