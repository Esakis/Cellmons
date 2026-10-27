# Rozwój ośmiu postaci w wersji testowej

Stan i plan z 20 października 2026. Po pięciu potwierdzonych klasach dodano Żniwiarza, Mędrca i Wampira; roboczy limit wynosi więc osiem.

## Zakres obecnego wdrożenia

**Dostępne:** Mag, Wojownik, Kapłan, Druid, Nekromantka, Żniwiarz i Mędrzec oraz nowy Wampir.

Rejestracja oferuje wybór **jednej z trzech klas: Mag, Wojownik, Kapłan** (decyzja właściciela z testu telefonu). Pozostałe pięć jest do zdobycia w grze. Trening offline pozwala wypróbować całą ósemkę bez przyznawania jej kontu. Każda klasa ma legalną talię 20 kart od poziomu 1 i pakiet 36–40 kart. Pierwsze utworzenie nowo odblokowanej klasy daje jej pakiet raz na konto. Usunięcie i ponowne utworzenie nie powiela kart. Część rezerwy wymaga późniejszych biegłości.

Pozostałe **24 bazowe postacie i ich drogi są zablokowane**: nie można ich wybrać, utworzyć ani użyć w nowej walce, otrzymać z areny, wypraw, wysypiska ani nowych turniejów. Poziom konta i inkarnacja nie znoszą blokady. Cienie także pozostają zamknięte. Włączenie nowej klasy wymaga jawnej aktualizacji `playable` w treści, pakietu i testów. Zachowujemy stare dane kolekcji i historii, żeby aktualizacja nie usuwała dorobku kont.

Na wysypisku, gdy brak dostępnej nieposiadanej klasy, działa istniejąca nagroda zastępcza w Dukatach. Pula treningu i nowych wypraw zawiera tylko osiem wydanych klas. Konta automatyczne przechodzą z wycofanego Runotechnika na zatwierdzone klasy; zachowują nick, wyniki i stare postacie.

Wampir korzysta z istniejących efektów: Kultysta wysysa życie co rundę, Nagrobny prowiant odzyskuje życie i wytrzymałość, a Kielich krwi później wysysa życie po trafieniu. Nie ma ukrytej, darmowej premii do każdego ataku. Jego dwie grywalne drogi to **Wampir Łowca** (fizyczny) i **Wampir Krwi** (magiczny); obie zachowują szkoły potrzebne do talii startowej.

**W kodzie po uwagach z telefonu:** trwały poziom konta 1–30, samouczek i serwerowe blokady lokacji. Rynek, Biblioteka i Trening są otwarte od początku; Arena od 3, Portal i czat wypraw od 4, Sklep od 6, Klan i Wieża od 10, Zamek od 20, Bazar od 22. Na początku mapa pokazuje Bibliotekę, Trening i zablokowaną Arenę. Po pierwszym treningu pokazuje pozostałe lokacje jako wyszarzone z progiem poziomu.

Samouczek przedstawia postać, talię i pierwszy pojedynek z podpowiedziami. Wprowadzenie daje raz 100 PD konta, otwierając edycję talii na poziomie 2. Pierwszy ukończony trening daje 150 PD i otwiera Arenę na poziomie 3. Bez wcześniejszego wprowadzenia trening przyznaje 250 PD, więc łączna nagroda pozostaje taka sama. Późniejsze treningi nie dają PD. Ukończona walka online inna niż trening daje 60 PD konta i +30 za wygraną; wymagane są co najmniej dwie rundy, bez poddania, rozłączenia i bezczynności. Konta automatyczne nie zdobywają PD konta. Zapis następuje w transakcji rozliczenia meczu, a ponowienie rozliczenia nie powiela nagrody. Migracja 29 przelicza historię kwalifikujących walk istniejących graczy, zapewniając im co najmniej poziom 3 i zakończony samouczek; migracja 31 uzupełnia 100 PD kontom będącym już w trakcie wprowadzenia. Kolekcje pozostają zachowane.

**Wdrożone w aktualizacji 1.0.4:** kontrakty na 30 fragmentów, opanowanie kart 1–5, zadanie trzech walk dziennie, dwie inkarnacje srebro/złoto z +3 i kolejnymi +3 trwałymi PR, próby klasy i drogi, jednorazowe pakiety dróg oraz osobna arena rankingowa. Poziomy postaci pozostają w zakresie 1–20; progi poziomu 20 to 6000/9000/12000 PD dla brązu/srebra/złota. Dodatkowy wymóg 65 000 PD usunięto. Stare dary i historia trzeciej inkarnacji pozostają zachowane. Poniższa tabela indywidualnych nazw złotych specjalizacji jest nadal propozycją nazewnictwa; obecna gra pokazuje istniejące, konkretne efekty specjalizacji.

## Zdobywanie kolejnych postaci — wdrożone

Na poziomach konta **5, 8, 12, 16, 20, 24 i 28** gracz otrzymuje prawo wyboru jednej nieposiadanej klasy z wydanej ósemki. W Bibliotece/Kolekcji otwiera panel „Rekrutacja postaci”, ogląda rolę i pełną talię wybranej klasy, a następnie potwierdza wybór. Powstaje postać na poziomie 1 z legalną talią 20 kart i jednorazowym pakietem klasy. Dotychczasowa aktywna postać pozostaje aktywna. Pełne 10 miejsc zatrzymuje odbiór bez zużycia nagrody.

Wcześniejsze nagrody z Areny lub wypraw zmniejszają listę brakujących klas, zachowując wszystkie progi rekrutacji. Po zebraniu całej ósemki gracz odbiera pamiątkową „Pieczęć kolekcjonera” danego poziomu, wyświetlaną przy odebranych nagrodach. To ozdoba kolekcji bez premii do parametrów. Wykrzyknik na Bibliotece sygnalizuje oczekujące odbiory. Nagrody nie wygasają; wcześniejsze progi są dostępne także istniejącym kontom.

Migracja 30 dodaje `recruitment_claims` z unikalnym kluczem konto + próg. Wybór, odblokowanie klasy, utworzenie postaci, pakiet i zapis odbioru są jedną transakcją z blokadą konta. Identyczne ponowienie zwraca aktualny stan bez kolejnej nagrody, a inny wybór dla wykorzystanego progu kończy się konfliktem. Usunięcie odebranej postaci nie odnawia nagrody. Eksport danych obejmuje historię odbiorów; wymazanie konta ją usuwa.

Od poziomu konta 4 działa drugi sposób: **kontrakt na wybraną brakującą klasę**. Wymaga 30 fragmentów: kwalifikująca walka daje 1, wygrana dodatkowy 1, a wygrany etap wyprawy jeszcze 1. Aktywny jest jeden kontrakt; zmiana celu zachowuje poprzedni postęp. Odbiór tworzy postać z talią i pakietem, nie zużywając nagród poziomowych. Pełne miejsca zatrzymują odbiór bez straty fragmentów. Blokada konta i trwały zapis odbioru chronią przed równoległymi żądaniami oraz ponownym przyznaniem nagrody po usunięciu postaci.

## Edytor talii po uwagach z telefonu

Układ nawiązuje do [Deck Buildera Cellmons Guru](https://www.cellmons.waw.pl/deck_builder.php): osobne obszary „Talia” i „Dostępne karty”, podgląd czaru i przykładowe rozdanie. Na telefonie obszary są przełączane zakładkami, na szerokim ekranie stoją obok siebie. Miniatury mają liczniki kopii oraz przyciski +/−, podgląd pokazuje efekt, koszty i wymagania szkół. Dostępne są filtry posiadania, szkoły, rzadkości i możliwości rzucenia czaru, cofnięcie ostatniej zmiany oraz próbna ręka. Zasady pozostają klasyczne: 20 kart i najwyżej 4 kopie; zachowany serwis pokazuje wariant 30 kart, którego nie przejmujemy.

## Rozwój każdej klasy w obecnej treści

AF/OF = atak/obrona fizyczna, AM/OM = atak/obrona magiczna, Ż = życie, W = wytrzymałość. Każda z ośmiu klas otrzymuje zwykłe punkty rozwoju: poziom 5 +1, 10 +2, 15 +1, 20 +2 (łącznie 6). Rekomendacje poniżej są wskazówkami do pierwszych testów, nie obowiązkowym rozdaniem.

### Mag

- **Rola:** Kombinacje zaklęć, koncentracja i kontrola zasobów.
- **Parametry AF/OF/AM/OM/Ż/W:** poziom 1 — 2 / 0 / 3 / 3 / 30 / 45; poziom 20 bez rozdanych PR — 4 / 2 / 10 / 8 / 90 / 125.
- **Kamienie milowe szkół:** poz. 1: Arkana 1; poz. 5: Arkana 2; poz. 9: Runotechnika 1; poz. 13: Arkana 3; poz. 17: Runotechnika 2. Zmiana dopisuje lub podnosi wskazaną szkołę; nie usuwa wcześniej zdobytych biegłości.
- **Wybór parametrów:** AM, OM i W. Ma chronić się przed fizyczną presją.
- **Nauka talii:** Wzmacnianie ataku magicznego i osłony; później wybrane karty Runotechniki.

### Wojownik

- **Rola:** Prosty, równy nacisk fizyczny. Uczy ataku, obrony i oszczędzania wytrzymałości.
- **Parametry AF/OF/AM/OM/Ż/W:** poziom 1 — 4 / 3 / 3 / 0 / 30 / 30; poziom 20 bez rozdanych PR — 10 / 8 / 6 / 3 / 110 / 100.
- **Kamienie milowe szkół:** poz. 1: Oręż 1; poz. 5: Oręż 2; poz. 8: Żywioły 1; poz. 12: Oręż 3; poz. 16: Żywioły 2. Zmiana dopisuje lub podnosi wskazaną szkołę; nie usuwa wcześniej zdobytych biegłości.
- **Wybór parametrów:** AF, OF, potem Ż lub W. Początkowo słaba obrona magiczna.
- **Nauka talii:** Utrzymać podstawową talię; karty Żywiołów dokładać dopiero po osiągnięciu ich wymagań.

### Kapłan

- **Rola:** Leczenie siebie, ochrona i wygrywanie dłuższych pojedynków.
- **Parametry AF/OF/AM/OM/Ż/W:** poziom 1 — 3 / 2 / 3 / 2 / 30 / 40; poziom 20 bez rozdanych PR — 8 / 5 / 9 / 4 / 89 / 109.
- **Kamienie milowe szkół:** poz. 1: Światłość 1; poz. 4: Światłość 1, Oręż 1; poz. 6: Światłość 2, Oręż 1; poz. 11: Światłość 2, Oręż 2; poz. 15: Światłość 3, Oręż 2. Zmiana dopisuje lub podnosi wskazaną szkołę; nie usuwa wcześniej zdobytych biegłości.
- **Wybór parametrów:** W i OM, następnie AM. Mniejsza zdolność szybkiego zakończenia walki.
- **Nauka talii:** Najpierw nauka osłony i leczenia; od Oręża 1 można dodać broń jako alternatywę.

### Druid

- **Rola:** Ofensywa żywiołów połączona z ochroną i regeneracją.
- **Parametry AF/OF/AM/OM/Ż/W:** poziom 1 — 3 / 0 / 4 / 2 / 35 / 30; poziom 20 bez rozdanych PR — 7 / 3 / 11 / 5 / 110 / 95.
- **Kamienie milowe szkół:** poz. 1: Żywioły 1; poz. 5: Żywioły 2; poz. 9: Oręż 1; poz. 13: Żywioły 3; poz. 17: Oręż 2. Zmiana dopisuje lub podnosi wskazaną szkołę; nie usuwa wcześniej zdobytych biegłości.
- **Wybór parametrów:** AM, W i Ż. Nie ma być równie dobry w każdej dziedzinie.
- **Nauka talii:** Kamienny atak, ciernie i leczenie; później fizyczne wsparcie z Oręża.

### Nekromantka

- **Rola:** Osłabianie, odbieranie zasobów i stopniowe wyniszczanie.
- **Parametry AF/OF/AM/OM/Ż/W:** poziom 1 — 3 / 0 / 4 / 2 / 30 / 40; poziom 20 bez rozdanych PR — 6 / 3 / 11 / 5 / 90 / 120.
- **Kamienie milowe szkół:** poz. 1: Nekromancja 1; poz. 3: Nekromancja 2; poz. 5: Arkana 1, Nekromancja 2; poz. 9: Arkana 2, Nekromancja 2; poz. 14: Arkana 2, Nekromancja 3. Zmiana dopisuje lub podnosi wskazaną szkołę; nie usuwa wcześniej zdobytych biegłości.
- **Wybór parametrów:** AM, W i OM. Słaba obrona fizyczna wymaga odpowiedzi w talii.
- **Nauka talii:** Nekromancja 2 pojawia się szybko; Arkana pozwalają rozwinąć kontrolę. Unikać kopiowania fizycznej roli Wampira.

### Wampir

- **Rola:** Fizyczny drapieżnik odzyskujący życie. Wygrywa przez nacisk i kontrolowane odnawianie zdrowia.
- **Parametry AF/OF/AM/OM/Ż/W:** poziom 1 — 4 / 1 / 2 / 2 / 30 / 35; poziom 20 bez rozdanych PR — 9 / 6 / 6 / 5 / 100 / 105.
- **Kamienie milowe szkół:** poz. 1: Oręż 1, Nekromancja 1; poz. 5: Nekromancja 2; poz. 10: Oręż 2; poz. 15: Nekromancja 3; poz. 19: Oręż 3. Zmiana dopisuje lub podnosi wskazaną szkołę; nie usuwa wcześniej zdobytych biegłości.
- **Wybór parametrów:** AF, W i Ż. Ochrona fizyczna jest słaba; samo leczenie nie powinno gwarantować zwycięstwa.
- **Nauka talii:** 20 kart: 4 Brązowe Ostrze, 3 Zdradziecki Sztylet, 3 Kultysta, 4 Nagrobny prowiant, 3 Wir Dusz, 3 Groza. Identyfikatory są w starter_decks.toml. Kielich krwi w rezerwie wymaga Nekromancji 2.

### Żniwiarz

- **Rola:** Napastnik fizyczny odporny na magię. Bezpośrednia presja i przełamywanie zaklęć.
- **Parametry AF/OF/AM/OM/Ż/W:** poziom 1 — 5 / 0 / 1 / 3 / 40 / 30; poziom 20 bez rozdanych PR — 12 / 5 / 5 / 6 / 130 / 85.
- **Kamienie milowe szkół:** poz. 1: Oręż 1; poz. 10: Oręż 2; poz. 18: Oręż 3. Zmiana dopisuje lub podnosi wskazaną szkołę; nie usuwa wcześniej zdobytych biegłości.
- **Wybór parametrów:** AF i W, następnie OF. Brak bazowej obrony fizycznej wymaga odpowiedzi w talii.
- **Nauka talii:** Gotowa talia Oręża. Wyższe biegłości pojawiają się później niż u Wojownika; rekompensują to bazowe AF 5, OM 3 i Ż 40.

### Mędrzec

- **Rola:** Taktyk łączący walkę i żywioły. Dużo zdrowia, ograniczony zapas wytrzymałości.
- **Parametry AF/OF/AM/OM/Ż/W:** poziom 1 — 3 / 1 / 3 / 2 / 45 / 25; poziom 20 bez rozdanych PR — 8 / 6 / 7 / 5 / 115 / 95.
- **Kamienie milowe szkół:** poz. 1: Oręż 1, Żywioły 1; poz. 5: Oręż 2; poz. 10: Żywioły 2; poz. 15: Oręż 3; poz. 19: Żywioły 3. Zmiana dopisuje lub podnosi wskazaną szkołę; nie usuwa wcześniej zdobytych biegłości.
- **Wybór parametrów:** W i OF, następnie AF lub AM zależnie od wybranej drogi. Bazowe W 25 wymaga oszczędnego zagrywania kart.
- **Nauka talii:** Start z Orężem i Żywiołami. Gotowa talia żywiołów, później zamiany na karty fizyczne; obie szkoły dochodzą do poziomu 3.

## Kontry zamiast identycznych szans w każdej parze

Klasy mają zachować wyraźne przewagi i słabości. Nie wprowadzamy ukrytego mnożnika obrażeń „klasa przeciw klasie”. Kontry wynikają z ataków, obron, biegłości, tempa i talii. Gracz może odpowiedzieć zmianą kart, rozłożeniem PR i wyborem drogi.

Pomiar wersji testowej: `cargo run --release -p sim -- matrix --n 500`, poziom 1, gotowe talie, boty L1, 28 000 pojedynków, każda para na obu stronach. Remis liczy się jako pół punktu. Poniższe wyniki są obserwacją tej konfiguracji, nie prognozą wyników ludzi ani wszystkich poziomów.

| Klasa | Przykład korzystnej pary | Przykład trudnej pary | Średni wynik |
|---|---|---|---:|
| Mag | Kapłan | Żniwiarz, Mędrzec | 48,0% |
| Wojownik | Mędrzec | Druid | 46,6% |
| Kapłan | Wampir, Mędrzec | Nekromantka | 55,5% |
| Druid | Wojownik, Kapłan | Mag | 59,1% |
| Nekromantka | Kapłan | Wampir | 48,6% |
| Wampir | Nekromantka | Kapłan, Mędrzec | 42,0% |
| Żniwiarz | Mag | Kapłan | 45,8% |
| Mędrzec | Mag, Wampir | Wojownik, Kapłan | 54,2% |

Żadna klasa nie przekracza obecnego progu 60% w całej puli. Niektóre talie startowe mają bardzo ostre kontry: np. Wampir–Kapłan. Przed rankingiem trzeba sprawdzić możliwość realnej odpowiedzi przez zmianę talii. Celem nie jest 50/50 w każdej parze, ale brak klasy najlepszej na wszystko i brak wyniku przesądzonego samym wyborem postaci.

## Docelowe srebrne drogi i złote specjalizacje

Dwie srebrne drogi na klasę i wybór specjalizacji na złocie już działają. Poniższe indywidualne nazwy i role to propozycja przyszłej warstwy fabularnej, nie lista nowych umiejętności. Obecny interfejs pokazuje istniejące nazwy dróg i specjalizacji oraz rzeczywiste efekty zmniejszania kosztów. Wydanie obejmuje 16 dróg; wszystkie mają legalną talię na poziomie 1.

| Klasa | Pierwsza droga srebrna → wybory złote | Druga droga srebrna → wybory złote |
|---|---|---|
| Mag | **Żywiołów:** Piromanta (presja) / Władca burz (zakłócanie) | **Ducha:** Teurg (ochrona i leczenie) / Nekromanta (klątwy i odzysk życia) |
| Wojownik | **Berserkera:** Furiat (ryzyko i atak) / Gladiator (przewidywanie i odpowiedź) | **Strażnika:** Bastion (obrona) / Rycerz run (broń wsparta magią) |
| Kapłan | **Uzdrowiciela:** Hierofanta (leczenie) / Opiekun dusz (ochrona) | **Inkwizytora:** Egzorcysta (usuwanie efektów) / Sędzia światła (ofensywa) |
| Druid | **Gaju:** Strażnik lasu (regeneracja) / Mistrz cierni (wyniszczanie) | **Przemiany:** Zmiennokształtny (walka fizyczna) / Szaman burzy (magia) |
| Nekromantka | **Zarazy:** Siewczyni plag (utrzymane osłabienia) / Władczyni rozkładu (presja) | **Dusz:** Tkaczka dusz (zasoby) / Krwawa wyrocznia (wymiana życia) |
| Wampir | **Łowcy:** Nocny łowca (fizyczne trafienia) / Krwawy szermierz (broń i przeżywalność) | **Krwi:** Hemomanta (magia i odzysk życia) / Władca nocy (koncentracja i kontrola) |
| Żniwiarz | **Ostrza:** Kosiarz (stała presja) / Egzekutor (mocne serie) | **Wojny:** Łamacz pancerzy (osłabianie obrony) / Wojenny kolos (odporność) |
| Mędrzec | **Harmonii:** Strażnik równowagi (ochrona) / Mistrz odnowy (zasoby) | **Wiedzy:** Mistrz żywiołów (magia) / Mędrzec bitwy (hybryda fizyczna) |

Wyróżniki opieramy początkowo na istniejących efektach, szkołach i parametrach. Przemiana nie oznacza jeszcze mechaniki zmiany formy, a nazwa Egzekutor nie oznacza automatycznego zabicia celu. Każda specjalizacja ma zyskać jedną mierzalną przewagę i słabość, bez całkowitej odporności, nieskończonego leczenia i nieograniczonych mnożników.

**Brąz → srebro:** konto 12+, postać 20, próba klasy, wybór jednej z dwóch dróg i +3 trwałe PR. **Srebro → złoto:** konto 18+, srebrna postać 20, próba drogi, wybór jednej z jej dwóch specjalizacji i kolejne +3 PR (łącznie 6).

Inkarnacja resetuje poziom i PD postaci do początku cyklu; nie resetuje konta, kolekcji ani innych postaci. Trwałe PR są przechowywane oddzielnie od zwykłych punktów poziomu, zwracane do rozdania przy zmianie cyklu i przyznawane tylko raz. Zwykłe punkty nowego cyklu zdobywa się ponownie na poziomach 5/10/15/20. Przelicznik: 1 PR = +1 wybrany atak/obrona albo +7 Ż/W. Próba wymaga trzech wygranych z poprawnie rzuconym czarem na Arenie swobodnej lub w wyprawie, rozpoczętych już postacią poziomu 20 i kontem odpowiednio 12/18. Ranking nie zalicza próby, ponieważ wyrównuje postacie.

Każda droga otrzymuje legalną talię i pakiet 20 dopasowanych kart, raz na konto i drogę. Dawna talia trafia do osobnego archiwum niezależnego od limitu dziesięciu zestawów. Przy kolejnym użyciu tej samej drogi układana jest legalna talia z posiadanych kart; ich niedobór zatrzymuje całą operację bez utraty punktów i postaci. Migracja 31 zachowuje stary profil w `legacy_progression`, stare dary i przydzielone parametry oraz uzupełnia premie 2/4 do 3/6 PR. Cienie pozostają zablokowane.

## Poziom konta i odblokowania — wdrożone

Poziom konta 1–30 jest wspólny, trwały i nie podnosi siły w pojedynku. Osiem klas jest pulą całego wydania: na początku gracz wybiera jedną, potem zdobywa pozostałe. Konto testowe z telefonu podlega tym samym zasadom.

| Poziom konta | Funkcja lub nagroda |
|---:|---|
| 1 | Wybór Maga, Wojownika albo Kapłana, gotowa talia, uproszczona kolekcja i trzy lekcje |
| 2 | Edycja talii po wprowadzeniu; lekcja budowania talii |
| 3 | Arena swobodna po ukończeniu podstaw |
| 4 | Pierwsza wyprawa, kontrakty i zadanie dzienne |
| 5 | Druga wybrana, nieposiadana klasa z aktywnej ósemki i jej pakiet |
| 6 | Sklep za zdobywaną walutę |
| 7 | Opanowanie kart i zapis zestawów |
| 8 | Trzecia klasa z aktywnej ósemki i pakiet |
| 10 | Dołączenie do klanu i Wieża |
| 12 | Czwarta klasa i próba srebra przy postaci 20 w Kolekcji |
| 15 | Arena rankingowa z wyrównaniem siły i założenie klanu |
| 16 | Piąta klasa i pakiet |
| 18 | Próba złota przy srebrnej postaci 20 |
| 20 | Szósta klasa i Zamek z turniejami |
| 22 | Bazar i wymiana |
| 24 | Siódma klasa i pakiet |
| 25 | Trudniejsze wyprawy; nagrody wyłącznie z aktywnej zawartości |
| 28 | Ósma klasa i pakiet |
| 30 | Kosmetyczny tytuł w Kolekcji, bez odblokowania Cieni |

Mapa wyróżnia następny cel. Blokady są sprawdzane na serwerze, także przy bezpośrednim wywołaniu API. Arena rankingowa wymaga konta 15 i potwierdzonego adresu e-mail. Obaj gracze używają swojej talii oraz bazowej klasy poziomu 20, bez przydzielonych PR, drogi, specjalizacji i daru. Nie zmienia to zapisanej postaci. Talia pozostaje bez zmian, więc karty obcych szkół mogą wymagać przebudowy przed rankingiem. Arena swobodna nie zmienia punktów rankingu.

## Tempo i rozwój kart — wdrożone

Ukończony kwalifikujący się pojedynek daje +60 PD konta i +80 PD postaci; wygrana dodatkowo +30/+40. Wprowadzenie i pierwszy trening dają łącznie 250 PD konta. Nagrody postaci z wypraw zachowują zasady depozytu i odwrotu; wygrany etap dodaje nominalnie 120 PD do puli. Poddanie, bezczynność, trening offline i pokazowe walki samych kont automatycznych nie dają nagród za rozegraną walkę. Niepotwierdzone konto zachowuje dotychczasowy limit 3000 PD postaci; do dalszego rozwoju trzeba potwierdzić e-mail.

Łączne progi konta: 2=100, 3=250, 4=450, 5=700, 6=1000, 7=1400, 8=1850, 10=3050, 12=4700, 15=8000, 18=12000, 20=16000, 24=24500, 25=27000, 28=35500, 30=42000 PD. Pełna rosnąca tabela znajduje się w `crates/server/src/journey.rs` i ma test każdej granicy.

Poziom 20 postaci: 6000 PD w brązie, 9000 w srebrze, 12000 w złocie. Przy 50% wygranych i 2–4 minutach na mecz pierwsze 6000 PD to około 60 walk, czyli 2–4 godziny samej gry. To model, nie pomiar zachowania ludzi. Przykład `progression_audit` odtwarza te zasady wraz z progami konta i próbami; raport z założeniami znajduje się w przeglądzie aktualizacji.

Opanowanie czaru 1–5 jest wspólne dla wszystkich jego kopii. Progi 0/15/45/100/200 PD; za walkę +1 za obecność definicji w talii i +2 za co najmniej jedno poprawne zagranie, maksymalnie +3 na definicję. Odznaka na miniaturze i podgląd pokazują postęp. Każdy nowy poziom daje raz 2 Dukaty, bez zmiany obrażeń. Od poziomu konta 4 ukończenie trzech kwalifikujących walk dziennie daje raz 5 Dukatów; dzień zmienia się o 00:00 UTC.

## Pozostałe prace i weryfikacja

1. Dostroić hierarchię siły botów: dwa porównania nie osiągają dotychczasowych progów, choć macierz klas startowych je spełnia.
2. Zmierzyć rzeczywisty czas gry i odpowiedzi talii na silne kontry na wyższych poziomach; obecne tempo jest modelem.
3. Rozważyć indywidualne nazwy złotych specjalizacji z tabeli po testach, zachowując opis rzeczywistych efektów.
4. Sprawdzić nowy APK na fizycznym telefonie, szczególnie powrót po uśpieniu i domową sieć.
5. Dopiero po testach właściciel wybiera następną klasę. Żaden poziom ani losowanie nie znosi blokady pozostałych 24.

Weryfikacja: rejestracja każdą klasą, pełne walki klas i dróg, próby obejścia blokad przez API i WASM, ponowienie odbioru pakietu, usunięcie i odtworzenie postaci, zachowanie starych kolekcji, wyłączenie nagród z nieaktywnej puli oraz test Androida przez LAN.
