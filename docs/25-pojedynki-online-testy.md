# F7.2–F7.5: pojedynki online

Stan implementacji: 2026-10-15. Walka działa na autorytatywnym serwerze; trening offline nadal korzysta z tego samego silnika w WASM. Tryb gościa nie otwiera połączenia z API ani WebSocket i zachowuje dane wyłącznie w pamięci przeglądarki.

## Aktor i odtwarzanie

Każdy pojedynek ma aktora z kolejką 64 poleceń. Serwer utrzymuje najwyżej 1024 aktywnych aktorów. Ruch jest walidowany i zapisywany transakcyjnie przed potwierdzeniem. Numer rundy i zapisany własny ruch blokują powtórne przesłanie. Przeciwnik otrzymuje informację o gotowości, bez treści nieujawnionego ruchu.

Migracje 006–007 dodają prywatny dziennik wejść silnika, blokady graczy, dziennik rund, wyniki, historię rankingu i statystyki. Dziennik zawiera ziarno oraz niezmienne ustawienia obu stron. Nie jest publicznym API. Wznowienie odtwarza silnik z wejść, porównuje zapisane ruchy i sumy kontrolne rund, po czym kontynuuje walkę. Nie serializujemy wewnętrznej pamięci silnika.

Dzierżawa właściciela trwa 5 sekund. Zapis sprawdza właściciela, rewizję i aktualny czas bazy; stary proces nie może potwierdzić ruchu po przejęciu walki. Restart daje ponownie czas na połączenie i aktualną decyzję, zachowując przyjęte ruchy. Zwykłe rozłączenie nie zatrzymuje zegara rundy: 90 sekund na rundę, najwyżej 60 sekund po ruchu przeciwnika, 60 sekund na powrót. Trzeci kolejny brak ruchu kończy walkę walkowerem. Jednoczesne walkowery obu stron oznaczają remis.

Uszkodzony dziennik jednej walki nie zatrzymuje odtwarzania pozostałych. Serwer loguje jej identyfikator i pozostawia blokady do wyjaśnienia przez operatora; nie wymyśla wyniku ani nagród. Odtwarzanie wymaga tej samej wersji silnika i treści. Wdrożenie zmieniające te wersje wymaga wygaszenia starych walk lub zachowania starego wykonawcy — automatyzacja wdrożeń pozostaje w F14.

Pole `state_hash` jest 32-bajtową sumą SHA-256 z deterministycznej, 64-bitowej sumy stanu silnika. Służy wykrywaniu rozbieżności. Oddzielne zobowiązanie SHA-256 z pełnego ziarna jest publiczne od początku, a samo ziarno dopiero po zatwierdzeniu wyniku.

## Wynik i klient

Jedna transakcja blokuje wynik oraz graczy i zapisuje PD, awanse, PR, statystyki i ranking. Identyfikator walki zapewnia rozliczenie raz. Powtórzenie odczytu zwraca historyczny rating z końca tej walki. Transfer rankingu zachowuje sumę punktów również przy minimum 1. Ranking zmieniają walki arenowe między potwierdzonymi kontami; niepotwierdzone konto nie uczestniczy w rankingu. Trening zapisuje powtórkę, bez PD, rankingu i zmian statystyk.

Parametry rankingu, PD i rotacji botów pochodzą z `content/rules.toml`; progi poziomów z tabeli CSV. Ekran online zawiera trzy ataki i obrony, wybór czaru lub odrzucenia karty, podgląd kosztu z serwera, czas na decyzję, gotowość i połączenie przeciwnika, poddanie, wynik oraz dziennik. Wznowiony widok zawiera własny przyjęty ruch. Końcowy snapshot zawiera także rozliczenie, aby utracone pojedyncze powiadomienie nie pozostawiło ekranu na „zapisywaniu wyniku”.

W lobby dostępne są ranking, historia z paginacją i powtórki zakończonych walk. Osiem botów treningowych ma różne poziomy i postacie zmieniane co pięć minut. Decyzje botów mają cztery miejsca pracy i limit oczekiwania 50 ms; przeciążenie daje legalny ruch domyślny.

## Weryfikacja i poprawki przeglądu

Testy PostgreSQL/WebSocket obejmują pełną walkę, odmowę dostępu osobie trzeciej do aktywnej walki, ukrywanie ręki i nieujawnionego ruchu, odwróconą perspektywę, błędną rundę i powtórny ruch, powtórki dopiero po zakończeniu, ranking, rozliczenie raz oraz historyczne nagrody. Osobne scenariusze sprawdzają limity czasu, trening bez nagród, powrót po rozłączeniu i przegraną po upływie czasu powrotu oraz odrzucenie zapisu starego właściciela.

Test chaosu zabija rzeczywisty proces serwera po zapisaniu pełnej rundy i przyjęciu ruchu następnej rundy. Nowy proces przejmuje dzierżawę, odtwarza stan i zapisany ruch, a obaj klienci kończą tę samą walkę. Testy wzoru rankingu sprawdzają wszystkie siedem przykładów archiwalnych.

Rozszerzony test Playwright na Chromium i emulowanym Pixel 5 obejmuje prawdziwą rejestrację dwóch kont, pięć rund, wyłączenie sieci, odświeżenie strony z zapisanym ruchem, wynik, historię, powtórkę i trening. Zrzuty: `target/online-duel-chromium.png` i `target/online-duel-mobile-chromium.png`. W przeglądzie poprawiono argumenty tłumaczeń zdarzeń, teksty i układ kart na telefonie, wyścig odczytu końcowego snapshotu, trwałość historycznych nagród oraz dostarczanie informacji o odzyskaniu i zakończeniu aktora.

Budżet offline zachowuje limit 200 KiB. Wydawany WASM pomija sekcje nazw diagnostycznych i producentów; oryginalny plik Cargo pozostaje do diagnostyki. Komentarze źródłowe Fluent nie trafiają do paczki. Ostatni pomiar: około 188 KiB offline i 20 KiB części konta/lobby pobieranej na żądanie.

Walidacja odbywa się lokalnie. Próba dwóch osób przez publiczny Internet wymaga środowiska testowego i pozostaje osobnym kryterium M3; test lokalny ani emulacja telefonu nie zastępują tego odbioru. Raport zbiorczy kontrolek powstaje w `target/f7-complete-check.log`; wynik należy czytać z końcowego podsumowania.
