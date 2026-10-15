# 11. AI i symulacje balansu

Crate `engine-ai` dostarcza boty, a `sim` narzędzia do symulacji balansu. Oba korzystają z szybkiego, deterministycznego silnika (`06-silnik-gry.md`).

## 1. Zastosowania

| Zastosowanie | Poziom bota | Gdzie działa |
|---|---|---|
| Sala treningowa (online) | L1–L3 według bota | serwer |
| Trening offline | L1–L3 do wyboru | przeglądarka (WASM w workerze) |
| Przeciwnicy w wyprawach | L1 (Spacer), L2 (Wyprawa), L3 (Męka, bossowie) | serwer |
| Symulacje balansu | L1–L3 | `sim` (natywnie, wiele wątków) |
| Testy obciążeniowe i fuzzing | L0–L1 | `loadtest`, testy silnika |
| Samouczek | L1 ze scenariuszem | przeglądarka |

## 2. Natura gry z punktu widzenia AI

- **Ruchy jednoczesne i ukryte:** to gra z niepełną informacją, podobna do „papier–kamień–nożyce” z ekonomią. Optymalna gra wymaga **strategii mieszanych** (losowania z rozkładu), bo przewidywalny bot jest łatwy do ogrania.
- **Mała przestrzeń ruchów:** 2³ ustawień ataków × 2³ ustawień obron × do 11 akcji kartą (brak, 5 × rzuć, 5 × odrzuć) = najwyżej 704 ruchy na gracza. Ograniczenia K i W tę liczbę dodatkowo zmniejszają.
- **Kluczowa obserwacja:** koncentracja przeciwnika jest jawna, więc wiadomo, ile slotów może on zmienić (R-RUN-05). Gracz z K = 1 może zmienić najwyżej jeden slot albo rzucić tani czar. To silne ograniczenie przewidywań i bot powinien z niego korzystać.
- **Informacje ukryte:** ręka i talia przeciwnika. Widoczne: jego postać, statystyki, umagicznienia, rzucone wcześniej czary i ustawienia z poprzedniej rundy.

## 3. Poziomy botów

### L0: losowy

Losuje jeden z dozwolonych ruchów z rozkładu równomiernego. Służy do fuzzingu i testów obciążeniowych.

### L1: heurystyczny

1. **Obrony:** ustawia obrony przeciw typowi ataku, który zrobi największą szkodę (porównanie `AF_przeciwnika − OF_własne` z `AM_przeciwnika − OM_własne`, z uwzględnieniem domyślnych ustawień przeciwnika).
2. **Ataki:** celuje w słabszą obronę przeciwnika (typ z większym `atak − obrona`).
3. **Koncentracja:** zmienia sloty tylko, gdy oczekiwany zysk przewyższa koszt; zostawia zapas na czar.
4. **Czary:** rzuca kartę o najwyższej „wartości na koszt” (wartość z heurystyki rodziny efektu, `03-tresc-gry.md`, rozdz. 5.3). Pilnuje, żeby W nie spadło poniżej progu bezpieczeństwa (R-ATK-03).
5. **Losowość:** z prawdopodobieństwem ε (domyślnie 0,15) zmienia jeden slot losowo, żeby nie być w pełni przewidywalnym.

**Implementacja (F2.6, `crates/engine-ai`).** `Bot::new(poziom, ziarno)` i `Bot::choose(&PlayerView, &Content) -> Move`; to samo ziarno i ten sam widok dają ten sam ruch. L0 wylicza wszystkie 64 ustawienia razem z akcjami kart i losuje spośród dozwolonych. L1 przewiduje, że przeciwnik zostawi ustawienia z poprzedniej rundy: atakuje typem, którego przeciwnik nie blokuje, a broni się przed jego ostatnimi atakami. Slot zmienia, gdy zysk w Ż przekracza dwukrotność kosztu zmiany. Czar wybiera według wartości efektu na koszt, z rezerwą W ≥ ¼ W_max, a kartę, której nie może rzucić, odrzuca.

Poprawki z kroku F3.5: obrony L1 ustawia przeciw typowi, który zada więcej obrażeń (zgodnie z punktem 1 wyżej), a nie tylko przeciw atakom z poprzedniej rundy; bez tego bot nie bronił się w 1. rundzie przed oczywistym atakiem w słabszą obronę. Punkt ataku lub obrony z karty jest wart ok. 1 Ż na każdą pozostałą rundę (wcześniej 3), bo większość ataków jest blokowana; zawyżona wartość sprawiała, że bot wybierał wzmocnienia zamiast obrażeń.

### L2: adaptacyjny (model przeciwnika)

- Model: dla każdego slotu rozkład wyborów przeciwnika warunkowany numerem rundy i jego ustawieniem z poprzedniej rundy, z rozkładem a priori Dirichleta. Model aktualizuje się w trakcie pojedynku, a dla graczy z historią (sala treningowa) także między pojedynkami.
- Ograniczenie przez K: ruchy, na które przeciwnika nie stać, mają prawdopodobieństwo 0.
- Odpowiedź: najlepsza odpowiedź na przewidywany rozkład, wybierana przez softmax z temperaturą τ (domyślnie τ = 0,3). Daje to zachowanie silne, ale nie w pełni deterministyczne.

### L3: przeszukiwanie (gra macierzowa)

Dla każdej rundy:
1. **Kandydaci:** po N ruchów na stronę (domyślnie 24), wybranych heurystykami L1/L2 tak, żeby pokryć różne typy zachowań (agresywne, obronne, z czarem, oszczędzające K).
2. **Determinizacja ukrytej ręki:** K próbek ręki przeciwnika (domyślnie 8) losowanych z puli kart zgodnych z jego szkołami i poziomem, ważonych popularnością kart w danym archetypie.
3. **Wypłaty:** dla każdej pary ruchów i każdej próbki silnik rozstrzyga rundę, a stan wynikowy jest oceniany funkcją oceny. Opcjonalnie dochodzi krótka symulacja do końca pojedynku botami L1. Daje to 24 × 24 × 8 ≈ 4600 symulacji rundy, czyli ok. 25 ms natywnie przy ≈ 5 µs na rundę.
4. **Rozwiązanie:** strategia mieszana przez regret matching (ok. 1000 iteracji na macierzy 24 × 24, poniżej 1 ms).
5. **Wybór:** losowanie ruchu ze strategii mieszanej.

**Funkcja oceny stanu** (wagi strojone symulacjami):

```
V = w1·(Ż%_własne − Ż%_przeciwnika) + w2·(W_własne/W_max − W_p/W_max_p)
  + w3·(K_własne − K_przeciwnika) + w4·wartość_umagicznień + w5·wartość_ręki
  + w6·(pozostałe_rundy-zależna premia za przewagę)
stan końcowy: wygrana +1, remis 0, przegrana −1
```

Wagi dobiera się przez rozgrywki bot przeciw botowi (przeszukiwanie siatki albo CMA-ES w `sim tune`).

### Uczciwość botów

Bot dostaje wyłącznie `PlayerView` (`06-silnik-gry.md`, rozdz. 8), czyli te same informacje co człowiek. Nie zna ręki przeciwnika, kolejności talii ani ziarna. API `engine-ai` nie przyjmuje `DuelState`, więc pilnuje tego system typów.

## 4. Profile i osobowości

Plik `content/bots.toml` definiuje boty:

| Pole | Znaczenie |
|---|---|
| `level` | L0–L3 |
| `aggression` | skłonność do ataków i ryzyka (wpływa na kandydatów i wagi w2/w3) |
| `spell_bias` | jak chętnie rzuca czary zamiast oszczędzać K |
| `epsilon` / `temperature` | stopień nieprzewidywalności |
| `deck_archetype` | archetyp talii (niżej) |
| `avatar_pool` | postacie, którymi bot może grać (rotacja, R-TRE-02) |

Archetypy talii (z poradnika graczy S9): „zjadanie wytrzymałości”, „gra na koncentracji”, „umagicznienia”, „zmasowany atak”, „kumulacja obrażeń”, „odbieranie kart”, „jedna karta”. Każdy archetyp to szablon talii dopasowywany do szkół postaci bota.

## 5. Przeciwnicy w wyprawach

| Trudność | Bot | Poziom przeciwników względem gracza | Boss |
|---|---|---|---|
| Spacer | L1 | −3 … 0 | L2, +1 |
| Wyprawa | L2 | −1 … +2 | L3, +2 |
| Męka | L3 | 0 … +3 | L3, +3, talia „bossa” |

Kolor nazwy przeciwnika (R-WYP-02): zielony, gdy jest niżej od gracza, żółty na równym poziomie, pomarańczowy przy +1…+2, czerwony od +3. Parametry są w `content/expeditions.toml`.

## 6. Symulator balansu (`sim`)

### 6.1. Polecenia

| Polecenie | Co robi | Wynik |
|---|---|---|
| `sim duel` | pojedyncza walka z pełnym zapisem zdarzeń | log tekstowy, powtórka |
| `sim run --n 100000 --a L1 --b L0` | seria pojedynków botów (od F2.6, na kartach testowych) | wygrane, remisy, powody końca, średnia liczba rund, przewaga inicjatywy, czas |
| `sim matrix --n 2000 --level 1 --bot L1` | od F3.5: postacie startowe z taliami startowymi z `content/`, każda przeciw każdej, ze zmianą stron co pojedynek | macierz odsetków wygranych (remis = ½), wynik każdej postaci z 95% przedziałem ufności; kod wyjścia 1, gdy któraś postać przekracza 60% |
| `sim duel 3 --starters dune_wanderer,ember_drake` | od F3.5: jeden pojedynek postaci startowych opisany runda po rundzie | log tekstowy |
| `sim matrix --level 10 --bot L2 --n 2000` | każda postać przeciw każdej na tym samym poziomie | macierz współczynników wygranych z przedziałami ufności |
| `sim card <klucz>` | test A/B: talia z kartą i bez niej | zmiana współczynnika wygranych, częstość użycia, wygrane po rzuceniu |
| `sim initiative` | przewaga inicjatywy | odsetek wygranych gracza z inicjatywą |
| `sim progression` | gracz o 50% wygranych: czas do poz. 20 i do reinkarnacji przy wzorze Z-XP | liczba walk i godzin gry |
| `sim economy` | dzienne źródła i ujścia Dukatów dla profili graczy | przepływy, czas do zestawu kart |
| `sim tournament --players 32` | przebieg turnieju na botach | czas trwania, rozkład wyników |
| `sim tune` | strojenie wag funkcji oceny L3 | nowe wagi i raport |

Symulator działa wielowątkowo (`rayon`). Każde uruchomienie jest powtarzalne dzięki ziarnu podanemu w parametrach.

### 6.2. Metryki i progi

| Metryka | Próg ostrzegawczy | Próg blokujący (CI dla treści) |
|---|---|---|
| Współczynnik wygranych postaci przeciw polu (ten sam poziom, L2) | > 55% albo < 45% | > 60% albo < 40% |
| Przewaga inicjatywy | > 55% | > 60% |
| Odsetek remisów | > 10% | > 20% |
| Zmiana wygranych po dodaniu jednej karty do talii | > 3 p.p. | > 6 p.p. |
| Mediana długości pojedynku | < 3 rundy | — |

Przedziały ufności liczy się metodą Wilsona. Wyniki trafiają do raportu Markdown, który CI dołącza do PR-a zmieniającego treść.

### 6.3. Przewaga inicjatywy

Inicjatywa jest stała przez cały pojedynek (R-POJ-05), więc może dawać trwałą przewagę: pierwszy czar, pierwszy cios, szybsze wyczerpanie W przeciwnika. Symulacja `sim initiative` na pełnej treści sprawdza to w fazie F3. Jeśli przewaga przekroczy próg blokujący, rozważamy kompensatę (np. +1 K na start dla gracza bez inicjatywy) jako nowe założenie projektowe z wpisem w rejestrze (`02-zasady-gry.md`, rozdz. 15).

## 7. Wydajność

| Miara | Cel natywnie | Cel WASM |
|---|---|---|
| Decyzja L1 | < 0,1 ms | < 0,5 ms |
| Decyzja L2 | < 1 ms | < 5 ms |
| Decyzja L3 (domyślne parametry) | ≤ 50 ms | ≤ 150 ms (w workerze, gracz tego nie odczuwa) |
| `sim matrix` 31 × 31 postaci × 2000 walk (L1) | < 2 min na 8 rdzeniach | — |

## 8. Testy AI

- Bot nigdy nie zgłasza niedozwolonego ruchu (test właściwościowy na losowych stanach).
- Te same ziarno i stan dają ten sam ruch.
- Regresja siły: nowa wersja bota nie może być słabsza od poprzedniej o więcej niż 3 p.p. w meczu bezpośrednim (1000 walk).
- Hierarchia: L3 > L2 > L1 > L0 z istotnością statystyczną (kryteria w F4, `16-plan-wdrozenia.md`).
