# Klany: implementacja i przegląd

F11.4, 2026-10-15. Katalog, profile i zarządzanie klanami działają dla kont w PL/EN. Gość widzi bramkę funkcji online, bez pobierania modułu klanów ani wywołań jego API. Dane gościa pozostają wyłącznie w RAM frontendu.

Klan powstaje z mistrzem i staje się założony po osiągnięciu pięciu członków. Późniejszy spadek liczebności nie cofa założenia. Limity wynoszą 256 członków i 10 równoczesnych podań jednej osoby. Nazwy rozwiązanych klanów pozostają zarezerwowane. To jawne decyzje uzupełniające R-KLA, a nie odtworzone szczegóły oryginalnej gry.

Zmiany blokują wiersze użytkowników w kolejności UUID, następnie klan; sprawdzają aktualną rolę i rewizję. Mistrz przekazuje przywództwo atomowo, v-mistrz nie zarządza mistrzem, skryba zmienia tylko opis i wiadomość dnia. Ostatni mistrz może rozwiązać klan; mistrz z innymi członkami najpierw przekazuje rolę. Dziennik zmian jest dopisywany i chroniony przed edycją oraz usunięciem.

Ranking jest całkowitą średnią do dziesięciu najlepszych aktywnych graczy klanu, liczoną przed filtrowaniem katalogu; remisy rozstrzyga UUID. Profil pokazuje publiczny kontakt klanu i sumę portfeli. Adres e-mail profilu nie ujawnia adresów kont. Wiadomość dnia jest prywatna, a lista kandydatów dostępna tylko mistrzowi i v-mistrzom.

Po zmianie członkostwa serwer odświeża obecność online. Usunięty członek opuszcza prywatny pokój, a każda kolejna próba wejścia lub nadania wiadomości ponownie sprawdza bazę. Niepowodzenie odświeżenia po zatwierdzonej transakcji nie zmienia wyniku operacji HTTP w pozorny błąd.

Przegląd poprawił zaokrąglenie rankingu, odpowiedzi błędów JSON, ochronę szkiców formularza, zmianę konta podczas pobierania, odświeżenie rozwiązanego profilu, kontrolę uprawnień i mobilne pola formularzy. Limity API obejmują też niepoprawne żądania uwierzytelnionego klienta.

Pięć testów PostgreSQL obejmuje cały cykl klanu, prywatność, role, pojemność, ranking i serie po 100 konkurujących przyjęć, przekazań oraz ponowień tworzenia. Test HTTP sprawdza sesję, CSRF, nieznane pola i idempotencję. Test WebSocket usuwa członka podczas pobytu w prywatnym pokoju i potwierdza brak dostępu do nowych wiadomości. Test przeglądarkowy dwóch kont obejmuje podanie, przyjęcie, skrybę, szkic formularza, przekazanie, usunięcie i rozwiązanie na desktopie oraz Pixel 5.

Pełna kontrola: 71 testów PostgreSQL, 25 Vitest, 24 testy przeglądarkowe, natywne/WASM, budżety i Lighthouse 99/100/100/100. Jedyny błąd pierwszego przebiegu dotyczył trzech zbędnych dereferencji w kodzie testu; po poprawce Clippy całego workspace przechodzi. Pozostałe kontrole nie wymagały zmian. Moduły offline zajmują około 199,3/200 KiB, moduły konta i lobby 41,1/45 KiB.

Turnieje klanowe pozostają osobnym punktem F11.2. Raport nie zastępuje ich testu akceptacyjnego.
