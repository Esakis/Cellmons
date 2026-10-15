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

**Implementacja F4.1 (`engine-ai/src/adaptive.rs`).** Model zlicza przejścia osobno dla sześciu slotów, poprzedniego typu i trzech faz walki (rundy 1–3, 4–6, 7–9). Prior Dirichleta 17:3 przewiduje zachowanie heurystyczne; obserwacje stopniowo zmieniają ten rozkład. Po każdym rozstrzygnięciu, także ostatniej rundy, wywołujący przekazuje `bot.observe(&view)`. Obserwacja jest idempotentna; ucieczka, rozłączenie i walkower nie dopisują nieujawnionego ruchu. `choose()` również aktualizuje model przed decyzją. Rozkład jest wspólny dla całych ustawień i odrzuca wszystkie, których koszt przekracza jawne K przeciwnika. Bot ocenia legalne ruchy, pomijając rzucenia kart o niespełnionych warunkach działania; osobne warunki wygranej są nadal oceniane na koniec rundy. Softmax ma deterministyczne przybliżenie stałoprzecinkowe `(1 + x/32)^32`. `model()` / `set_model()` pozwalają zachować historię jednego przeciwnika; `reset_duel()` usuwa przejście między walkami, zachowując statystyki.

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

**Implementacja F4.2 (`engine-ai/src/search.rs`).** Do 24 kandydatów zawiera ruch bez zmian, najlepszy ruch dla każdej dostępnej akcji kartą i pozostałe ruchy według oceny L2. Bot losuje 8 możliwych układów kart, uwzględniając szkoły, jawne karty i limit kopii; jawne odrzuty zwiększają wagę popularności kart. Przy braku dostatecznej puli szkolnej uwzględnia uzupełnienia z całej puli ze zwracaniem (R-TAL-05), bez ograniczania ich limitem kopii. Bez losowego uzupełniania respektuje limit także dla kart spoza szkół (R-TAL-04). Każdy kandydat własny jest ten sam we wszystkich próbkach. Kolumny przeciwnika są politykami wyboru według rankingu, więc mogą zależeć od jego wylosowanej ręki. Macierz sumuje oceny po `resolve_round`, a dwustronny regret matching wykonuje 1000 iteracji i zwraca średnią strategię. Funkcja oceny uwzględnia procent Ż, W, K, statystyki w pozostałych rundach, umagicznienia i karty w ręce. Wynik końcowy ma pierwszeństwo przed oceną zasobów. To przeszukiwanie jednej rundy; rollouty i `sim tune` pozostają rozszerzeniami.

Algorytm regret matching opisują [Neller i Lanctot (2013)](https://www.cs.gettysburg.edu/~tneller/modelai/2013/cfr/index.html). Ograniczenia próbkowania informacji ukrytych, w tym problem łączenia strategii z różnych determinizacji, omawiają [Cowling, Powley i Whitehouse (2012)](https://eprints.whiterose.ac.uk/id/eprint/75048/1/CowlingPowleyWhitehouse2012.pdf).

### Uczciwość botów

Bot dostaje wyłącznie `PlayerView` (`06-silnik-gry.md`, rozdz. 8), czyli te same informacje co człowiek. Nie zna ręki przeciwnika, kolejności talii ani ziarna. API `engine-ai` nie przyjmuje `DuelState`, więc pilnuje tego system typów.

`PlayerView::sample_state` buduje hipotetyczny stan z jawnego snapshotu, dostarczonych kart i nowego ziarna bota. Snapshot zachowuje warstwy statystyk i skalowane efekty umagicznień, ale usuwa obie ręce, obie talie i RNG. Własną rękę uzupełnia dopiero z jawnego pola `hand` widoku. Testy porównują widoki i decyzje przy różnych prawdziwych kartach, kolejnościach talii i ziarnach.

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

**Implementacja F4.3.** `training_profiles()` wczytuje wbudowane `content/bots.toml`: Nowicjusz (L0, `balanced`), Uczeń (L1, `burst`), Taktyk (L2, `endurance`) i Arcymag (L3, `enchantment`). Nazwy są w PL i EN. `profile.bot(seed)` konfiguruje osobowość; `profile.player(level, rotation)` wybiera postać z puli i buduje pełną talię zgodną z jej szkołami i limitem kopii. Wartości `aggression` i `spell_bias` są procentami 0–200 (100 neutralne), `epsilon_percent` to 0–100, a `temperature_milli` to 1–1000. Są to profile dla przyszłego ekranu treningu F5. Nieznane pola, powtórzone identyfikatory, błędne zakresy i odwołania do nieistniejących postaci są odrzucane; wszystkie profile i talie są sprawdzane testami w CI.

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
| `sim ai --n 1200 --level 1 --seed 0` | F4: cztery serie L2–L1 i L3–L2, osobno boty domyślne z taliami startowymi oraz profile z własnymi taliami; jednakowe postacie z części wspólnej pul, rotacja stron | wygrane (remisy nie są wygranymi), 95% CI Wilsona, mediana/p95/max decyzji; kod 1, gdy którakolwiek seria nie spełnia 60% / 55% lub p95 L3 > 50 ms w release |
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

Testy F4 sprawdzają także uczenie się po ujawnieniu rundy (łącznie z ostatnią), brak podwójnego zliczania i uczenia z ucieczek, przenoszenie modelu między walkami, zerowe prawdopodobieństwo ustawień przekraczających K, zachowanie interwencji do spełnienia warunku karty, próbki talii z losowymi uzupełnieniami, mieszanie strategii na grze papier–kamień–nożyce i wybór ruchu dominującego. `engine-wasm/tests/ai.rs` porównuje stały skrót pełnych pojedynków L2–L3 natywnie i w Node/WASM. Testy symulatora sprawdzają, że serie profili używają ich talii oraz jednakowych postaci po obu stronach.

Pomiar po przeglądzie F4 (2026-10-15, silnik 0.2.0, Windows, release, `sim ai --n 1200`, poziom postaci 1, ziarno 0):

| Scenariusz | Wygrane / remisy / porażki | Wygrane (95% CI) | p95 decyzji |
|---|---|---|---|
| Domyślne L2–L1, talie startowe | 766 / 243 / 191 | 63,8% (61,1–66,5%) | 0,08 ms |
| Domyślne L3–L2, talie startowe | 689 / 343 / 168 | 57,4% (54,6–60,2%) | 9,4 ms |
| Taktyk–Uczeń, talie profili | 759 / 231 / 210 | 63,2% (60,5–65,9%) | 0,07 ms |
| Arcymag–Taktyk, talie profili | 672 / 323 / 205 | 56,0% (53,2–58,8%) | 8,6 ms |

Przed strojeniem Taktyk z parametrami 90/110/300 osiągał 58,4% w scenariuszu CI. Po strojeniu ma `aggression = 200`, `spell_bias = 80`, `temperature_milli = 1000`; Arcymag ma `aggression = 130`. Dodatkowe serie po 2400 walk od niezależnego ziarna 100000 dały Taktykowi 1507 zwycięstw (62,8%), a Arcymagowi 1326 (55,3%). Parametry Ucznia i jego talia nie zostały osłabione.

To wyniki dla treści startowej na poziomie 1; zmiany treści i dodatkowe poziomy postaci wymagają osobnych pomiarów. `check.bat` i CI powtarzają wszystkie cztery scenariusze z tabeli i blokują zmianę, jeśli choć jeden nie spełnia progu.
