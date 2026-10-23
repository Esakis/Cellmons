# Przegląd zmian mobilnych i progresji — 20 października 2026

Zakres: cały pakiet zgłoszeń z telefonu oraz nowe kontrakty, rozwój kart i inkarnacje. Przegląd kodu objął rejestrację, samouczek, mapę, czat i pocztę, edytor talii, bramki poziomu konta, transakcje nagród, ranking, migrację i prywatność. To przegląd zmian i ich integracji z istniejącymi funkcjami, nie niezależny audyt całego produktu.

## Wykryte i poprawione

| Priorytet | Problem | Poprawka i dowód |
|---|---|---|
| P1 | Sprawdzenie pola `playable` bezpośrednio na drodze blokowało wszystkie pakiety dróg. | Dostępność dziedziczona z klasy bazowej przez `is_playable`; test wymaga dokładnie 16 dróg i sprawdza legalność każdej talii na poziomie 1. |
| P1 | Rozliczenie szybkiego poddania mogło zwiększać postęp Areny i przyznawać klasę. | Nagrody za rozegraną walkę wymagają co najmniej dwóch rund i normalnego zakończenia. Test rzeczywistej piątej wygranej potwierdza jedną nagrodę i odporność na ponowne rozliczenie; osobny test sprawdza brak nagrody za poddanie. Walkower nadal rozstrzyga turniej i ranking. |
| P1 | Odczyt tylko ostatniej rundy pomijał wcześniej zagrane czary, a zdarzenie próby rzucenia mogło zaliczać nieudany czar. | Odtworzenie wszystkich rund z dziennika i odrzucenie `SpellFailed` w każdej z nich; wcześniejszy udany czar pozostaje zaliczony. Kopie kart nie mnożą PD. |
| P1 | Ranking mógł zaufać nieaktualnemu statusowi potwierdzenia konta z pamięci lobby. | Ponowne pobranie profili i kontrola poziomu 15 oraz weryfikacji pod blokadą kont w transakcji przyjęcia wyzwania. Test WebSocket porównuje postać nową i dawną z darem, sprawdzając identyczne parametry oraz nienaruszony zapis postaci. |
| P1 | Ograniczenie bazy odrzucało nowe rodzaje wypłat. | Migracja rozszerza dozwolone powody księgowania o zadanie dzienne i opanowanie karty; integracyjny test rozlicza faktyczne walki, wypłaty i ponowienia. |
| P2 | Zmiana cyklu mogła pozostawić niegrywalną talię albo stracić stary zestaw przy pełnych miejscach. | Pakiet raz na konto i drogę, legalna talia 20 kart, osobne archiwum poprzednich talii, transakcyjny powrót do starego stanu przy niedoborze kart. |
| P2 | Dotychczasowe punkty inkarnacji nie były oddzielone od zwykłych PR. | Migracja zapisuje dawny profil, zachowuje przydzielone parametry i dary, uzupełnia premie do 3/6 i rozdziela pulę niewydaną. Nowy cykl zwraca trwałe PR do rozdania. Test aktualizacji z migracji 30 sprawdza stan i ponowienie migratora. |
| P2 | Oznaczenie opanowania karty kolidowało z dostępną nazwą przycisku podglądu. | Odznaka jest osobnym tekstem w kafelku karty; przycisk zachowuje zgodną nazwę. Scenariusze kolekcji uruchamiają axe bez wyłączania reguł. |
| P2 | Nowe dane progresji nie były objęte eksportem i wymazaniem konta. | Dodano osiem sekcji danych; testy sprawdzają rozdzielenie kont oraz usunięcie nowych zapisów bez naruszania księgi walutowej. |

Odbiór kontraktu i nagrody poziomowej jest serializowany blokadą konta. Sprawdzono równoległe odbiory, ponowienia po utracie odpowiedzi, pełne dziesięć miejsc i usunięcie odebranej postaci. Przełączenie kontraktu zachowuje fragmenty. Zablokowane lokacje, edycja talii i zapis zestawów są kontrolowane także przy bezpośrednim żądaniu do API.

## Otwarte ustalenia

**P2 — hierarchia botów nie spełnia dwóch progów jakości.** Symulacja 1200 walk na porównanie daje 58,1% dla domyślnego L2 przeciw L1 przy wymaganych 60% oraz 52,7% dla Arcymaga przeciw Mistrzowi Bitew przy wymaganych 55%. Kod AI nie został zmieniony w tym pakiecie; taki sam problem był odnotowany przed nim. Pozostałe dwa porównania i limit czasu decyzji przechodzą. Nie obniżono progów testu. W rezultacie całego `check.bat` nie należy przedstawiać jako zaliczonego. Zwykłe rozgrywanie pojedynków i macierz klas startowych mają osobne pozytywne wyniki.

**Granica potwierdzenia mobilnego:** przeglądarka emuluje rozmiary telefonu i dotyk, lecz nie potwierdza zachowania konkretnego urządzenia, uśpienia aplikacji ani domowego routera. Nowy APK wymaga próby na fizycznym telefonie.

Indywidualne nazwy i nowe role 32 złotych specjalizacji z tabeli projektowej pozostają propozycją. Wdrożono dwa cykle rozwoju na istniejących 16 drogach i istniejących efektach specjalizacji. Nie ma nowych, nieopisanych premii bojowych ani odblokowanych Cieni.

## Wyniki kontroli

Pełny przebieg: `target/advancement-full-check.log`. Początkowe błędy oczekiwań starych nagród oraz dostępności zostały poprawione i sprawdzone w przebiegach uzupełniających. Log początkowy jest zachowany, aby nie ukrywać wcześniejszych niepowodzeń.

- Rust: formatowanie, Clippy wszystkich celów, testy workspace, walidacja treści i zgodność silnika natywnego z WASM. Końcowy lint i test modelu tempa: `target/advancement-static-final.log`.
- PostgreSQL: 135 testów zaliczonych, w tym migracje, konta, kontrakty, inkarnacje, klany, wyprawy, lobby, prywatność i operacje: `target/advancement-db-final.log`. Osiem testów turniejów oraz kontrola SQLx przeszły po korekcie oczekiwań zadania dziennego i walkoweru: `target/advancement-tournaments-complete.log`.
- Klient: 38 testów Vitest, typy klienta i E2E oraz Biome; `target/advancement-client-final.log`.
- Przeglądarka: 24 scenariusze zaliczone w `target/advancement-e2e-verified.log`, następnie oba rozbudowane scenariusze lobby po poprawce etykiet w `target/advancement-lobby-verified.log` — łącznie wszystkie 26. Pokrywają rejestrację, samouczek i blokady, wiadomości między dwoma klientami, talię, dwie inkarnacje, kontrakt i prawdziwy odbiór postaci. Polski i angielski układ mapy sprawdzono przy 320 × 640, 390 × 844 i 412 × 915. Wcześniejszy jednorazowy timeout przygotowania cache offline przy ośmiu procesach nie powtórzył się przy dwóch; nie zmieniono limitu oczekiwania ani asercji.
- Macierz 28 000 walk ośmiu klas na poziomie 1 spełnia limit średniego wyniku 60%; najwyższy wynik ma Druid: 59,1%. Nie oznacza to sprawdzonego balansu wszystkich poziomów i talii ludzi.
- Lighthouse w pełnym przebiegu: wydajność 97, dostępność 100, dobre praktyki 100, SEO 100.
- Rozmiary klienta pozostają pod dotychczasowymi limitami. Kontrakty są ładowane osobno razem z pozostałymi funkcjami online; limitów nie zwiększono w tym pakiecie.
- Android: zbudowano APK 1.0.4 / 10004, zweryfikowano metadane i podpis v2, TLS, gotowość lokalnej bazy oraz identyczność pobranego pliku. Zachowano konto testowe. Szczegóły i suma SHA-256: [testy na telefonie](40-testy-na-telefonie.md). ADB nie wykazało podłączonego urządzenia, więc nowy APK nie ma potwierdzenia z fizycznego telefonu.

## Model tempa

Polecenie: `cargo run -p server --example progression_audit --locked`. Wynik: `target/advancement-pacing.json`. Sto deterministycznych symulacji; potwierdzone konto, 50% zwycięstw, trzy minuty na walkę, trzydzieści minut gry dziennie. Każda walka online kwalifikuje się do nagrody i zawiera udany czar. Uwzględniono pierwszy trening; pominięto kolejki i czas w menu.

| Cel | Mediana liczby walk, łącznie | Mediana czasu samej gry |
|---|---:|---:|
| Arena | 1 trening | około 3 min |
| Brąz, poziom 20 | 61 | 3,1 h |
| Srebro po próbie | 68 | 3,4 h |
| Złoto po próbie | 164 | 8,2 h |
| Złoto, poziom 20 | 285 | 14,3 h |

To model zasad, nie telemetria ludzi. Dla srebra zakres P10–P90 wyniósł 64–73 walki, dla złota 159–169. Test wariantu samych zwycięstw sprawdza zarówno progi konta, jak i trzy zwycięstwa wymagane po osiągnięciu poziomu 20.
