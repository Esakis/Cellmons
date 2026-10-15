# 02. Specyfikacja zasad gry (wersja klasyczna)

Dokument opisuje zasady, które implementuje silnik i serwer. Odtwarza klasyczną wersję Cellmons z lat 2004–2009, uzupełnioną o decyzje projektowe tam, gdzie źródła milczą. To **jedyne źródło prawdy dla logiki gry**: kod i testy odwołują się do identyfikatorów reguł (`R-…`).

## 0. Konwencje

### 0.1. Oznaczenia pochodzenia reguły

| Znacznik | Znaczenie |
|---|---|
| `[Ź:S2]` | Fakt potwierdzony w źródle (kody S1–S12, patrz `01-badania-i-zrodla.md`, rozdz. 4) |
| `[W]` | Wniosek z danych, np. z zapisów walk (S5); spójny ze źródłami, ale niepodany wprost |
| `[Z-XXX]` | Założenie projektowe, bo źródła milczą. Zawsze ma parametr konfiguracyjny (rozdz. 13) i wpis w rejestrze (rozdz. 15) |
| `[D]` | Świadoma decyzja produktowa, odmienna od oryginału (np. brak płatności) |

### 0.2. Skróty

| Skrót | Znaczenie | Identyfikator w kodzie |
|---|---|---|
| AF / OF | atak / obrona fizyczna | `phys_atk`, `phys_def` |
| AM / OM | atak / obrona magiczna | `mag_atk`, `mag_def` |
| Ż | punkty życia | `hp` |
| W | punkty wytrzymałości | `stamina` |
| K | punkty koncentracji | `focus` |
| PD | punkty doświadczenia | `xp` |
| PR | punkty rozwoju | `dev_points` |
| F / M | typ ataku lub obrony: fizyczny / magiczny | `Kind::Phys`, `Kind::Mag` |

### 0.3. Arytmetyka

Wszystkie wartości w logice pojedynku i rozwoju postaci są **liczbami całkowitymi**. Ułamki (procent życia, mnożniki) liczy się na liczbach całkowitych z jawnie określonym zaokrągleniem. Silnik nie używa liczb zmiennoprzecinkowych (determinizm, patrz `06-silnik-gry.md`). Prawdopodobieństwa nagród (szansa na postać, zawartość zestawów) losuje serwer poza silnikiem i mogą być zapisane w konfiguracji jako ułamki.

---

## 1. Pojęcia i encje

**R-ENT-01. Konto gracza.** Konto ma pulę postaci, kolekcję kart, Dukaty, ranking i statystyki (gry, wygrane/remisy/przegrane, rozłączenia) `[Ź:S1]`.

**R-ENT-02. Szkoły Esencji.** Jest 6 szkół: Walka, Kapłaństwo, Elementalizm, Magia, Nekromancja, Technologia `[Ź:S2,S3]`. W oryginale Technologia doszła z dodatkiem; u nas jest od początku `[D]`.

**R-ENT-03. Biegłość w szkole** ma poziom 0–4: 0 brak, 1 Nowicjusz, 2 Adept, 3 Mistrz, 4 Arcymistrz `[Ź:S3]`. Postać może mieć biegłość w kilku szkołach naraz.

**R-ENT-04. Szablon postaci** (karta postaci) definiuje: nazwę, kategorię rzadkości, statystyki na poz. 1, tabelę przyrostów na poz. 2–20, poziomy szkół na każdym poziomie, harmonogram punktów rozwoju, dwie drogi reinkarnacji oraz specjalizacje i dary dla każdej drogi `[Ź:S6,S8]`.

**R-ENT-05. Postać** (instancja) to szablon plus stan: poziom 1–20, PD, poziom reinkarnacji (0 brak, 1 Adept, 2 Mistrz, 3 Ascent), wybrana droga, specjalizacja i dar, rozdysponowane i niewydane PR, przypisana talia `[Ź:S1,S3]`.

**R-ENT-06. Statystyki postaci:** AF, OF, AM, OM, Ż (maksimum), W (maksimum) `[Ź:S1]`. Statystyka efektywna = baza z tabeli rozwoju + wydane PR + premie reinkarnacji. W pojedynku dochodzą modyfikatory z czarów.

**R-ENT-07. Karta czaru** ma: identyfikator, rzadkość (brązowa, srebrna, złota), wymagania szkół (od 0 do 2 szkół z poziomami 1–4), koszt W, koszt K, typ i słowa kluczowe (rozdz. 5) oraz opis efektu `[Ź:S2,S7]`. Kilka kart nie ma wymagań, ale zwykle są kosztowne `[Ź:S2]`.

---

## 2. Talia, ręka, kolekcja

**R-TAL-01.** Każda postać ma własną talię (zestaw czarów). Talia ma do **20 kart** `[Ź:S1,S2]` (`deck.size`).

**R-TAL-02.** W talii może być najwyżej **4 kopie** tej samej karty `[Ź:S1,S3]` (`deck.max_copies`).

**R-TAL-03.** Do talii można włożyć tylko karty, które gracz ma w kolekcji, w liczbie nie większej niż posiadana `[Ź:S1]`. Ta sama karta może być w taliach kilku postaci jednocześnie `[W]`.

**R-TAL-04.** Do talii można dodać kartę, której postać nie może rzucić (np. z myślą o efektach podnoszących szkoły) `[Ź:S2: przycisk „wszystkie”]`.

**R-TAL-05.** Jeśli talia ma mniej niż 20 kart, na początku pojedynku brakujące miejsca wypełnia się kartami losowanymi z puli wszystkich kart gry (ze zwracaniem) `[Ź:S1,S3]` (`deck.fill_missing_with_random = true`).

**R-TAL-06.** Gracz może zapisać kilka nazwanych zestawów i szybko je wczytywać `[Ź:S1,S2]` (`deck.saved_presets_max`, domyślnie 10 `[Z-PRE]`).

**R-TAL-07.** Kolekcja to liczba egzemplarzy każdej karty. Karty nie są związane z postacią: usunięcie postaci nie usuwa kart `[Ź:S2]`.

**R-TAL-08.** Jeśli przez wymianę lub wysypisko gracz ma mniej egzemplarzy, niż jest w talii, talia jest automatycznie przycinana. Brakujące miejsca obsługuje R-TAL-05 `[Z-TRM]`.

---

## 3. Przygotowanie pojedynku

**R-POJ-01. Snapshot.** W chwili rozpoczęcia pojedynku zapisuje się kopię statystyk efektywnych obu postaci, poziomów szkół, specjalizacji i daru. Zmiany na koncie w trakcie pojedynku nie wpływają na walkę `[Ź:S1: po wyzwaniu nie można zmienić postaci ani czarów]`.

**R-POJ-02. Stan początkowy:** Ż = Ż_max, W = W_max, K = 0, wszystkie modyfikatory puste `[Ź:S2]`. `Ż_start` (maksimum Ż w chwili startu) służy później do liczenia procentu życia (R-KON-03).

**R-POJ-03. Tasowanie.** Talię każdego gracza tasuje się generatorem pseudolosowym pojedynku (z ziarnem `seed`). Brakujące karty losuje się według R-TAL-05 `[Ź:S1]`.

**R-POJ-04. Ręka.** Każdy gracz dobiera **5 kart** `[Ź:S1,S2]` (`duel.hand_size`).

**R-POJ-05. Inicjatywa.** Jeden z graczy dostaje inicjatywę **losowo, raz na cały pojedynek**. Gracz z inicjatywą w każdej rundzie pierwszy rozpatruje czar i pierwszy zadaje cios `[Ź:S2]`. Inicjatywa może się zmienić tylko w wyniku efektu karty.

**R-POJ-06. Ustawienia domyślne** (punkt odniesienia dla kosztu zmian w 1. rundzie):
- ataki: wszystkie 3 typu, który ma wyższą statystykę ataku (AF lub AM);
- obrony: wszystkie 3 typu, który ma wyższą statystykę obrony (OF lub OM);
- przy remisie statystyk: F `[Z-DEF]`.

Źródło: `[Ź:S2: „ustawione na fizyczne, ponieważ są to najwyższe statystyki postaci”; W:S5: walka #26519]` (`duel.default_settings = "highest_stat"`).

**R-POJ-07. Dar** (po 3. reinkarnacji) zaczyna działać przed 1. rundą, jak darmowe umagicznienie `[Z-GFT]`.

---

## 4. Przebieg rundy

Pojedynek trwa najwyżej **5 rund** `[Ź:S1,S2]` (`duel.rounds`). Każda runda składa się z faz:

```
Początek rundy → Decyzje (równocześnie, w ukryciu) → Ujawnienie
  → Faza czarów → Faza ataków → Koniec rundy → [warunki końca]
```

### 4.1. Początek rundy

**R-RUN-01.** Każdy gracz dostaje **+4 K** `[Ź:S1,S2]` (`duel.focus_per_round`).

**R-RUN-02.** Niewydana K przechodzi na kolejne rundy bez limitu `[Ź:S2]` (`duel.focus_cap = none`).

**R-RUN-03.** Licznik serii trafień (combo) każdego gracza zeruje się `[W]`.

### 4.2. Decyzje

**R-RUN-04.** Obaj gracze **jednocześnie i w ukryciu** ustawiają:
- 3 ataki: dla każdego slotu `k = 1..3` typ F lub M;
- 3 obrony: dla każdego slotu `k = 1..3` typ F lub M;
- akcję karty: brak, **rzucenie** jednej karty z ręki albo **odrzucenie** jednej karty z ręki `[Ź:S1,S2]`.

**R-RUN-05. Koszt zmian.** Każdy slot ataku lub obrony, który różni się od ustawienia z poprzedniej rundy (w 1. rundzie: od ustawień domyślnych R-POJ-06), kosztuje **1 K** `[Ź:S1,S2]` (`duel.change_cost`). Liczy się tylko stan końcowy w chwili zatwierdzenia; przełączenie slotu tam i z powrotem nic nie kosztuje `[W: opis interfejsu w S2]`.

**R-RUN-06. Walidacja ruchu.** Ruch jest poprawny, gdy:
1. K ≥ koszt zmian + koszt K rzucanej karty (po specjalizacji, R-CZA-09);
2. przy rzucaniu: W ≥ koszt W karty (po specjalizacji) `[Z-SPW]`;
3. przy rzucaniu: poziomy szkół postaci spełniają wymagania karty `[Ź:S1,S3]`;
4. przy rzucaniu interwencji: ta karta nie była jeszcze użyta w tym pojedynku (R-CZA-03);
5. odrzucenie karty jest darmowe `[Z-DSC]`.

Serwer odrzuca niepoprawny ruch i odsyła błąd. Gracz może poprawić ruch przed upływem czasu.

**R-RUN-07. Zatwierdzenie** jest ostateczne. Przeciwnik widzi tylko informację, że ruch został wysłany `[Ź:S1: „Wróg wysłał ruch”]`.

**R-RUN-08. Limit czasu.** Od chwili, gdy przeciwnik zatwierdzi ruch, gracz ma **60 s** `[Ź:S2]` (`duel.move_timeout_after_opponent_s`). Dodatkowo cała faza decyzji trwa najwyżej 90 s `[Z-TMO]` (`duel.round_timeout_s`).

**R-RUN-09. Brak ruchu.** Po upływie czasu przyjmuje się ruch „bez zmian”: ustawienia z poprzedniej rundy, bez akcji karty. Trzy kolejne braki ruchu tego samego gracza oznaczają walkower (przegrana, powód `timeout`) `[Z-TMO]` (`duel.max_consecutive_timeouts = 3`).

### 4.3. Ujawnienie i opłaty

**R-RUN-10.** Po zebraniu obu ruchów serwer ujawnia je obu graczom i pobiera K za zmiany ustawień `[Ź:S2]`.

### 4.4. Faza czarów

**R-RUN-11. Kolejność:**
1. aktywne umagicznienia działają w kolejności ich rzucenia (R-CZA-02);
2. przeciwzaklęcia rozstrzyga się według R-CZA-05;
3. nowe czary rozpatruje się w kolejności: najpierw gracz z inicjatywą, potem drugi `[Ź:S1,S2]`.

**R-RUN-12. Rzucenie czaru:** sprawdzenie, czy rzucający nadal może zapłacić (efekt przeciwnika mógł zmniejszyć jego W lub K). Jeśli nie może, czar przepada bez kosztów i bez efektu, a karta i tak opuszcza rękę `[Z-SPW]`. Jeśli może: pobranie W i K `[Ź:S1]`, wykonanie efektu (lub rejestracja umagicznienia), karta trafia na stos odrzuconych.

### 4.5. Faza ataków

**R-ATK-01. Kolejność.** Wykonuje się 6 ataków naprzemiennie, zaczyna gracz z inicjatywą: `I1, N1, I2, N2, I3, N3` (I: z inicjatywą, N: bez) `[Ź:S1,S2]`.

**R-ATK-02. Blok.** Atak gracza A w slocie `k` jest zablokowany, gdy obrona gracza D w tym samym slocie ma ten sam typ co atak **i** D ma W ≥ 1 `[Ź:S1,S2,S3]` (`duel.block_min_stamina = 1`). Efekty kart mogą tę regułę zmieniać (np. „atak nie może zostać zablokowany”).

**R-ATK-03. Brak wytrzymałości.** Jeśli w chwili ataku obrońca ma W = 0, atak trafia niezależnie od obrony `[Ź:S2,S3]`.

**R-ATK-04. Trafienie:**

```
seria_A   := seria_A + 1
mnożnik   := 2^(seria_A − 1)                  → 1, 2, 4      [Ź:S2,S3,S4]
atak      := AF_A dla ataku F, AM_A dla ataku M  (efektywny)
obrona    := OF_D dla ataku F, OM_D dla ataku M  (efektywna)
obrażenia := max(0, atak × mnożnik − obrona)  [Ź:S2: „nie można otrzymać obrażeń ujemnych”]
Ż_D       := Ż_D − obrażenia
W_A       := max(0, W_A − atak)               [Ź:S1,S2: koszt = wartość ataku, bez mnożnika i bez odejmowania obrony]
```

Trafienie z zerowymi obrażeniami nadal jest trafieniem: kosztuje W i podbija serię `[Ź:S2: przykładowa walka]`. Seria liczy kolejne trafienia **tego samego atakującego** w tej rundzie, niezależnie od typu ataku `[W]`. Mnożniki konfiguruje `duel.combo_multipliers = [1, 2, 4]`.

**R-ATK-05. Blok** zeruje serię atakującego (`seria_A := 0`) i nic nie kosztuje żadnej ze stron `[W:S5]`.

**R-ATK-06. Wszystkie 6 ataków wykonuje się zawsze**, nawet jeśli Ż którejś postaci spadnie do 0 lub niżej w trakcie fazy `[Ź:S2]`.

**R-ATK-07.** W nie spada poniżej 0. Ż może spaść poniżej 0 (wartość wewnętrzna), w interfejsie wyświetla się 0 `[Z-NEG]`.

### 4.6. Koniec rundy

**R-RUN-13.** Wyzwalają się efekty „na koniec rundy” (np. z umagicznień) `[W]`.

**R-RUN-14. Dobranie.** Każdy gracz, który w tej rundzie rzucił lub odrzucił kartę, dobiera jedną kartę z wierzchu talii na jej miejsce `[Ź:S2]`. Gdy talia jest pusta, nie dobiera `[Z-EMP]` (przy talii 20 kart i 5 rundach to się nie zdarza).

**R-RUN-15.** Sprawdzenie warunków końca (rozdz. 6).

---

## 5. Czary — typy i słowa kluczowe

**R-CZA-01. Czar zwykły (natychmiastowy).** Efekt wykonuje się raz, w chwili rzucenia. Może trwale zmienić statystyki do końca pojedynku (np. „+2 do AF”) `[Ź:S8]`. Takiej zmiany nie usuwają efekty rozpraszające umagicznienia `[Z-PRM]`.

**R-CZA-02. Umagicznienie.** „Czar działa w rundzie, w której został rzucony, i w każdej następnej w identyczny sposób” `[Ź:S2]`. Trwa do końca pojedynku, chyba że zostanie usunięte (rozproszenie) `[Ź:S9]`. Umagicznienie może też reagować na zdarzenia, np. „każdy udany atak twojej postaci daje jej Ż i W” `[Ź:S8]`. Lista aktywnych umagicznień każdej postaci jest jawna `[Ź:S2]`.

**R-CZA-03. Interwencja.** Kartę z tym słowem kluczowym można rzucić tylko raz w pojedynku `[Ź:S2,S4]`. Limit dotyczy nazwy karty: druga kopia tej samej interwencji jest zablokowana, inna interwencja nie `[Z-INT]`.

**R-CZA-04. Przeciwzaklęcie.** „Uniemożliwia rzucenie czaru przez przeciwnika” w tej rundzie `[Ź:S2]`.

**R-CZA-05. Rozstrzyganie przeciwzaklęć** `[Z-CNT]`:
1. Przeciwzaklęcia rozpatruje się przed pozostałymi czarami, w kolejności inicjatywy.
2. Przeciwzaklęcie anuluje czar przeciwnika z tej rundy. Rzucający przeciwzaklęcie płaci koszty. Czar anulowany: przeciwnik płaci jego koszty, ale efekt nie występuje.
3. Gdy obaj rzucają przeciwzaklęcia, działa tylko przeciwzaklęcie gracza z inicjatywą, a drugie zostaje anulowane.

Parametr `duel.counter_resolution = "initiative_first"`.

**R-CZA-06. Sprzężenie.** Karta ze sprzężeniem do szkoły X na poziomie p: jeśli postać ma X na poziomie ≥ p, za **każdy** poziom tej szkoły płaci dodatkowo 1 K i 5 W. Przykład: sprzężenie do poziomu 2 przy postaci na poziomie 4 to +4 K i +20 W `[Ź:S3]`. Efekt karty skaluje się z poziomem tej szkoły `[W]`.

**R-CZA-07. Karty losowe.** Np. karta typu „puszka z losowym czarem” zostaje w chwili rzucenia zastąpiona losową kartą z puli wszystkich kart. Rzucający płaci koszt W wylosowanej karty, a wymagania szkół są pomijane `[Ź:S3]`.

**R-CZA-08. Warunki.** Efekt karty może mieć warunek (np. „nie działa, jeśli otrzymano dokładnie 10 obrażeń albo obrażenia zadał czar”) `[Ź:S3]`. Gdy warunek nie jest spełniony, koszty są pobrane, a efektu brak `[Z-CND]`.

**R-CZA-09. Specjalizacja** (2. reinkarnacja) zmniejsza koszty o 1/3 `[Ź:S3]`:
- specjalizacja w szkole X: koszt W czarów, których wymagania obejmują X;
- „Esencja”: koszt W wszystkich czarów;
- „Koncentracja”: koszt K wszystkich czarów.

Zaokrąglenie: `koszt_po = (2 × koszt + 1) div 3`, czyli 1→1, 2→1, 3→2, 4→3, 6→4, 16→11 `[Z-SPC]`. Koszty sprzężenia (R-CZA-06) liczy się przed redukcją.

**R-CZA-10. Kategorie efektów** spotykane w oryginale `[Ź:S8,S9]` (lista dla projektantów treści, nie zamknięta):
- zmiana statystyk (+/− AF, OF, AM, OM, Ż_max, W_max);
- leczenie, obrażenia od czaru, przywracanie lub wysysanie W, zmiana K;
- manipulacja ustawieniami (zamiana ataków z obronami przeciwnika, wymuszenie trafienia, „pierwszy atak fizyczny przeciwnika nie trafia”, „wszystkie ataki trafiają”, „ataki fizyczne obu postaci trafiają”);
- zamiana statystyk F↔M;
- odrzucanie kart z ręki przeciwnika, dobieranie kart;
- usuwanie umagicznień (rozproszenie), umagicznienia obustronne;
- zmiana poziomów szkół (np. „wzmocnienie X”: +1 poziom szkoły X);
- efekty losowe (aury);
- efekty wygrywające warunkowo (ostatnie rundy).

Projekt i katalog efektów: `03-tresc-gry.md`, rozdz. 5.3 i 5.4. Implementacja: `06-silnik-gry.md`, rozdz. 6.

---

## 6. Koniec pojedynku

**R-KON-01. Nokaut.** Sprawdza się go raz, na końcu rundy, czyli po fazie ataków. Ataki wykonują się zawsze do końca (R-ATK-06), nawet jeśli Ż spadło do 0 już w fazie czarów:
- obie postacie mają Ż ≤ 0 → **remis** `[Ź:S2]`;
- jedna postać ma Ż ≤ 0 → przegrywa `[Ź:S1,S2]`.

**R-KON-02. Koniec rund.** Po 5. rundzie bez nokautu wygrywa postać z wyższym procentem życia `[Ź:S1,S2,S3]`.

**R-KON-03. Procent życia** = `Ż / Ż_start`. Porównanie bez zaokrągleń, na liczbach całkowitych: A wygrywa, gdy `Ż_A × Ż_start_B > Ż_B × Ż_start_A`. Przy równości remis `[Ź:S2]`. Ż ujemne liczy się jako 0. Procent może przekroczyć 100%, jeśli efekt zwiększył Ż ponad start.

**R-KON-04. Ucieczka.** Gracz może się poddać w dowolnej chwili. To przegrana z powodem `forfeit`, z normalnymi skutkami dla rankingu i PD `[Ź:S2: przycisk ucieczki; Z-FRF: skutki]`.

**R-KON-05. Rozłączenie.** Po utracie połączenia gracz ma **60 s** na powrót i kontynuację; potem przegrywa (powód `disconnect`) `[Ź:S2]` (`duel.reconnect_grace_s`). W tym czasie licznik ruchu (R-RUN-08) biegnie normalnie. Rozłączenia liczą się w statystykach gracza `[Ź:S1]`.

**R-KON-06. Wygrane warunkowe.** Efekt karty może zakończyć pojedynek natychmiast (np. „wygrywasz, jeśli…”) `[W:S8]`. Takie sprawdzenie następuje na końcu rundy, przed R-KON-01; jeśli spełnione dla obu stron, jest remis `[Z-IWN]`.

---

## 7. Nagrody za pojedynek

### 7.1. Ranking

**R-RNK-01.** Ranking ma gracz (nie postać). Wartość startowa: **1000** `[Z-RNK]` (`rating.initial`; w oryginale tytuł „student esencji” obejmował zakres 1000–1099).

**R-RNK-02.** Po pojedynku rankingowym zwycięzca zyskuje, a przegrany traci tę samą liczbę punktów `[Ź:S3,S4; W:S5]`:

```
Δ = floor( 10 × (R_przegranego / R_zwycięzcy) × (L_przegranego / L_zwycięzcy) )
```

R to ranking **przed** pojedynkiem, L to poziom postaci (1–20). Obliczenie na liczbach całkowitych: `Δ = (10 × R_p × L_p) div (R_w × L_w)`. Wzór zgadza się z 7 z 7 archiwalnych walk (`01-badania-i-zrodla.md`, rozdz. 5.1).

**R-RNK-03.** Remis nie zmienia rankingu `[Z-RDR]` (`rating.draw_delta = 0`).

**R-RNK-04.** Ranking zmieniają tylko pojedynki na arenie i w turniejach `[Ź:S2,S3]`. Trening, wyprawy i walki kont gościa go nie zmieniają.

**R-RNK-05.** Ranking nie spada poniżej 1 `[Z-RMN]`, bo wzór dzieli przez ranking zwycięzcy.

**R-RNK-06. Tytuły** wynikają z rankingu `[Ź:S3]`:

| Ranking | Tytuł | Ranking | Tytuł |
|---|---|---|---|
| 0–399 | niewolnik | 1300–1399 | mag |
| 400–599 | zdegradowany | 1400–1499 | czarownik |
| 600–799 | nieposłuszny | 1500–1599 | mentor |
| 800–999 | nowicjusz | 1600–1799 | mistrz |
| 1000–1099 | student esencji | 1800–1999 | lisz |
| 1100–1199 | adept esencji | 2000–2199 | arcymag |
| 1200–1299 | wtajemniczony | 2200–2499 | mistrz esencji |
| | | 2500+ | wybraniec |

Nazwy tytułów są generyczne, ale „wybraniec” i „esencja” to elementy fabuły oryginału. Do zmiany w treści gry (ADR-012).

**R-RNK-07.** Ranking klanu liczy się z rankingów członków `[Ź:S2]`: średnia z 10 najwyższych rankingów członków `[Z-KLR]`.

### 7.2. Doświadczenie

**R-PD-01.** PD zdobywa postać, która walczyła. PD nigdy nie maleją, z wyjątkiem reinkarnacji `[Ź:S2]`. Liczba PD zależy od siły przeciwnika i typu walki `[Ź:S2]`.

**R-PD-02. Wzór** `[Z-XP]`. Oryginalny wzór się nie zachował; wiadomo tylko, że zależał od poziomu i reinkarnacji obu postaci `[Ź:S3]`.

```
L_eff(postać) = poziom × M[tier] + A[tier] × 100      (w setnych częściach poziomu)
  M = [100, 110, 120, 130], A = [0, 0, 3, 6]           (tier 0..3; wycena postaci z S10)
PD_wygrana   = clamp( (BAZA × L_eff(przeciwnik)) div L_eff(własna), PD_MIN, PD_MAX )
PD_remis     = PD_wygrana div 2                        [Ź:S3: PD za wygraną lub remis]
PD_przegrana = 0
```

Przykład: Adept na poz. 10 (L_eff = 1100) pokonuje postać bez reinkarnacji na poz. 12 (L_eff = 1200): PD = 100 × 1200 div 1100 = 109.

Domyślnie `xp.base = 100`, `xp.min = 10`, `xp.max = 300`. Cień liczy się jak Ascent na tym samym poziomie. Parametry kalibrujemy symulacjami (`11-ai-i-symulacje.md`).

**R-PD-03.** Trening nie daje PD `[Ź:S2]`. Konto z niepotwierdzonym e-mailem ma limit PD na postać (R-KNT-02).

**R-PD-04.** Postać na poziomie 20 z 65 000 PD przestaje zdobywać PD `[Ź:S1,S2]`.

**R-PD-05.** Awans na kolejny poziom następuje natychmiast po przekroczeniu progu (tabela w `03-tresc-gry.md`, rozdz. 4) `[D]`. W oryginale PD z turniejów zaliczały się dopiero po walce poza zamkiem — tego nie odtwarzamy.

### 7.3. Nowe postacie za walkę na arenie

**R-NAG-01.** Po każdej wygranej na arenie jest szansa na otrzymanie **brakującego** szablonu postaci (zawsze takiego, którego gracz nie ma) `[Ź:S3]`. Średnio raz na ok. 200 walk; w pierwszych 50 walkach gracz dostaje 2–3 postacie „na zachętę” `[Ź:S3]`.
- Implementacja `[Z-DRP]`: szansa `unlock.arena_chance = 0.005` po wygranej, losowanie kategorii 70% pospolita / 25% niepospolita / 5% rzadka.
- Gwarantowane nagrody po wygranych nr 5, 15 i 30 (`unlock.newbie_wins = [5, 15, 30]`).
- Szablonów z wysypiska i cieni nie można zdobyć tą drogą `[Ź:S3]`.

---

## 8. Rozwój postaci

### 8.1. Poziomy i statystyki

**R-ROZ-01.** Postać ma poziomy 1–20. Progi PD podaje `docs/dane/progi_doswiadczenia.csv` `[Ź:S1,S3]`.

**R-ROZ-02.** Na każdym poziomie statystyki rosną według tabeli przyrostów szablonu: AF, OF, AM, OM, W, Ż oraz poziomy szkół `[Ź:S6]`. Przykład dla poz. 2: +1 AM, +3 W, +6 Ż.

**R-ROZ-03. Punkty rozwoju.** Szablon określa, na których poziomach postać dostaje PR. Typowo: 5 → +1, 10 → +2, 15 → +1, 20 → +2, razem 6 `[Ź:S6]`. Postacie z wysypiska dostają PR co 2 poziomy (10 łącznie) `[Ź:S6,S8]`.

**R-ROZ-04.** Jeden PR to +1 do AF, OF, AM lub OM albo **+7** do Ż_max lub W_max `[Ź:S2,S3]`. Wydanie jest natychmiastowe i nieodwracalne.

**R-ROZ-05.** Niewydane PR przepadają przy reinkarnacji `[Ź:S3]`.

### 8.2. Reinkarnacja

**R-REI-01. Warunek:** poziom 20 i 65 000 PD `[Ź:S1,S2]`. Reinkarnacja jest dobrowolna, a uruchamia ją gracz.

**R-REI-02. Skutek:** PD = 0, poziom = 1, wydane PR zostają wyzerowane, poziom reinkarnacji +1, premia PR rozdawana na poziomie 1 `[Ź:S1,S2]`. Premia rośnie z każdą reinkarnacją: Adept 2, Mistrz 4, Ascent 6 `[Z-REI]` (`progression.reincarnation_bonus_dp = [2, 4, 6]`).

**R-REI-03. 1. reinkarnacja (Adept):** gracz wybiera jedną z dwóch **dróg**. Droga zastępuje szablon rozwoju: nowe statystyki bazowe, nowa tabela przyrostów i nowe szkoły. Wybór jest trwały `[Ź:S2,S6,S8]`.

**R-REI-04. 2. reinkarnacja (Mistrz):** gracz wybiera jedną z dwóch **specjalizacji** dostępnych dla drogi (R-CZA-09) `[Ź:S2,S3,S6]`.

**R-REI-05. 3. reinkarnacja (Ascent):** gracz wybiera jeden z dwóch **darów** dostępnych dla drogi. Dar to stała zdolność w walce `[Ź:S2,S6]`, aktywna od początku pojedynku jak darmowe umagicznienie `[Z-GFT]`.

**R-REI-06.** Postać po 3. reinkarnacji nie ma kolejnych reinkarnacji. Może natomiast zostać poświęcona na cień (8.3) `[Ź:S3,S6]`.

### 8.3. Cienie

**R-CIE-01.** Cień powstaje przez poświęcenie postaci Ascent z 65 000 PD. Poświęcona postać znika z puli `[Ź:S3,S6]`. Gracz wybiera cień jednej z 6 szkół; wyboru nie można zmienić `[Ź:S6]`.

**R-CIE-02.** Statystyki cienia na poziomie 1: AF 2, OF 2, AM 2, OM 2, Ż 35, W 40, biegłość 1 w szkole cienia `[Ź:S8]`.

**R-CIE-03.** Przy awansie cień dostaje tylko przyrost Ż i W oraz 1 PR na poziom, a na poziomach 19 i 20 po 2 PR (21 łącznie). Ataki i obrony nie rosną automatycznie `[Ź:S6]`. Przyrost Ż/W: +4 Ż, +4 W na poziom `[Z-CIE]`.

**R-CIE-04. Reinkarnacja cienia** wymaga poziomu 20 i 65 000 PD oraz poświęcenia innej postaci, która ma o jeden poziom reinkarnacji więcej niż cień i nie ma żadnej biegłości w szkole cienia `[Ź:S6]`. Drogi cienia: „Obłęd” (szkoła cienia + szkoły przeciwne) i „Perfekcja” (szkoła cienia do poziomu 4) `[Ź:S6]`.

**R-CIE-05.** Cieni nie można zdobyć w turniejach, na arenie, w wyprawach ani na wysypisku `[Ź:S3]`.

### 8.4. Pula postaci

**R-PUL-01.** Gracz ma najwyżej **10 postaci**. W danej chwili aktywna jest jedna i to nią toczy pojedynki `[Ź:S1,S2]` (`avatars.max_slots`).

**R-PUL-02.** Nową postać tworzy się z szablonu, który gracz posiada (odblokował). Nowa postać zaczyna od poziomu 1 i 0 PD `[Ź:S1,S2]`. Ten sam szablon może posłużyć do wielu postaci `[Ź:S3]`.

**R-PUL-03.** Usunięcie postaci jest nieodwracalne. Aktywnej postaci nie można usunąć `[Ź:S2]`.

**R-PUL-04.** Szablonów postaci nie można wymieniać, sprzedawać ani przekazywać `[Ź:S3]`.

**R-PUL-05. Szablony zdobywa się:** na starcie (postać startowa wybranej szkoły), na arenie (R-NAG-01), w turniejach o postać, w wyprawach (R-WYP-05) i na wysypisku (R-WYS-02). Cienie powstają tylko według rozdz. 8.3 `[Ź:S3]`.

---

## 9. Tryby gry

### 9.1. Arena

**R-ARN-01.** Pojedynki z innymi graczami na podstawie wyzwań (rozdz. 10.2). Rankingowe, dają PD i szansę na postać `[Ź:S2,S3]`.

### 9.2. Sala treningowa

**R-TRE-01.** W sali treningowej są boty z prefiksem „AI” na różnych poziomach. Wyzwanie bota jest przyjmowane automatycznie, a bot wysyła ruch od razu. Walka toczy się na normalnych zasadach, bez wpływu na PD, ranking i statystyki `[Ź:S1,S2]`.

**R-TRE-02.** Co kilka minut boty zmieniają postacie `[Ź:S2]` (`training.bot_rotation_min = 5` `[Z-BOT]`).

**R-TRE-03.** U nas trening działa też offline, w przeglądarce, bez serwera `[D]` (ADR-001).

### 9.3. Wyprawy (Portal)

**R-WYP-01.** Tryb dla jednej osoby przeciw przeciwnikom komputerowym. Poziomy trudności: Spacer (łatwy), Wyprawa (normalny), Męka (trudny) `[Ź:S2]`. Dostępny tylko dla kont zarejestrowanych `[Ź:S3]`.

**R-WYP-02.** Wyprawa składa się z etapów. Na każdym etapie gracz spotyka grupę 3 przeciwników i wybiera jednego do walki. Po wygranej przechodzi dalej. Kolor nazwy przeciwnika podpowiada trudność (różnicę poziomów) `[Ź:S2]`. Liczba etapów: Spacer 4, Wyprawa 6, Męka 8, plus boss `[Z-EXP]`.

**R-WYP-03.** Na końcu czeka główny przeciwnik (boss). Z nim walczy się bez wyboru `[Ź:S2]`.

**R-WYP-04.** Przegrana kończy wyprawę. Gracz może się też wycofać, zachowując część PD zależną od postępu `[Ź:S2]`: `PD_zachowane = PD_zebrane × ukończone_etapy / wszystkie_etapy` `[Z-EXP]`. Wyprawy nie zmieniają rankingu `[Z-EXP]`.

**R-WYP-05. Nagroda** po pokonaniu bossa `[Ź:S3]`: pełne PD plus losowanie:
- 40% — szablon postaci, której gracz nie ma: Spacer daje pospolitą, Wyprawa niepospolitą, Męka rzadką;
- 3% — karta;
- 1% — Dukaty (Spacer 10, Wyprawa 25, Męka 50 `[Z-EXP]`);
- w pozostałych przypadkach nic ponad PD.

U nas dochodzi stała nagroda Dukatów za ukończenie (rozdz. 11.1) `[D]`.

**R-WYP-06.** Podczas wyprawy gracz może rozmawiać z innymi graczami na wyprawach, ale każdy toczy własne walki `[Ź:S2]`.

### 9.4. Turnieje (Zamek)

**R-TUR-01. Twórca** ustala: liczbę graczy, wpisowe (Dukaty), maksymalną liczbę walk w fazie (standardowo 3), nagrodę, opcje klanowe (tylko mój klan lub turniej klanowy z listą klanów), zakres poziomów postaci i dopuszczalne poziomy reinkarnacji `[Ź:S2]`.

**R-TUR-02. Rodzaje** `[Ź:S3]`:
- tworzone przez graczy: o PD (bez wpisowego) albo o Dukaty i PD (z wpisowym; całe wpisowe trafia do puli nagród `[Ź:S2]`);
- tworzone przez system lub administratora (bez wpisowego): o postać, zestaw kart, kartę lub Dukaty.

Cieni i postaci z wysypiska nie można wygrać `[Ź:S3]`.

**R-TUR-03. Rejestracja.** Postać musi spełniać warunki turnieju. Turniej startuje po zebraniu kompletu graczy `[Ź:S1,S2]`. W trakcie turnieju nie można zmienić postaci `[Ź:S1]`.

**R-TUR-04. System pucharowy.** Pary losuje się na starcie. Zwycięzca pary przechodzi do następnej fazy `[Ź:S1,S2]`. Gdy liczba graczy nie jest potęgą dwójki, wolne losy (bye) dostają losowo wybrani gracze `[Z-TUR]`.

**R-TUR-05. Faza** to seria do N walk (standardowo 3). Wygrana daje 2 punkty, remis po 1. Fazę wygrywa ten, kogo przeciwnik nie może już dogonić. Przy remisie po N walkach zwycięzcę się losuje `[Ź:S2]` (`tournament.tiebreak = "random"`; wariant 2010 `"less_experienced"`).

**R-TUR-06. Harmonogram.** Pierwsza walka rusza ok. 20 s po starcie turnieju, każda następna 15 s po powrocie obu graczy do pokoju turniejowego `[Ź:S2]`. Gracz, który nie przystąpi do walki w wyznaczonym czasie, przegrywa ją walkowerem `[Ź:S1: limit gotowości; Z-TUR: walkower]`.

**R-TUR-07.** Walki turniejowe dają PD i zmieniają ranking jak na arenie `[Ź:S2]`. Zwycięzca turnieju dostaje nagrodę `[Ź:S2]`.

**R-TUR-08. Turniej klanowy:** walczą klany. Indywidualna wygrana daje klanowi 2 punkty, remis po 1. Gdy klan zdobędzie dość punktów, do następnej fazy przechodzą wszyscy jego gracze, a gracze przegranego klanu odpadają (nawet jeśli wygrali swoje walki). Remis klanów rozstrzyga losowanie. Z każdego klanu musi się zapisać tyle samo osób `[Ź:S2]`.

**R-TUR-09.** Turniej można opuścić przed startem (zwrot wpisowego). Po starcie opuszczenie oznacza walkower we wszystkich pozostałych walkach `[Z-TUR]`.

---

## 10. Konta, lobby, komunikacja, klany

### 10.1. Konta

**R-KNT-01.** Rejestracja jest darmowa: login, e-mail, hasło, wybór szkoły startowej. Wybór szkoły daje zestaw startowy kart (`docs/dane/zestawy_startowe.csv`) i postać startową szkoły `[Ź:S2,S6; D: bez opłat]`. Wyboru szkoły nie można zmienić `[Ź:S6]`.

**R-KNT-02. Konto z niepotwierdzonym e-mailem** ma ograniczenia `[Ź:S3; D: kryterium = potwierdzenie e-maila zamiast płatności; nazwa uściślona w ADR-018]`:
- postacie przestają zdobywać PD po 3000 PD;
- brak rankingu (walki z kontem z niepotwierdzonym e-mailem nie zmieniają rankingu żadnej ze stron);
- brak turniejów, wymiany i wypraw;
- czat z ograniczeniem częstotliwości i bez wiadomości prywatnych do nieznajomych `[D]`.

**R-KNT-03.** W oryginale ważność konta była płatna. **Nie odtwarzamy tego** `[D]`.

**R-KNT-04. Gość lokalny** wchodzi bez rejestracji i logowania. Jego dane i postęp istnieją wyłącznie w pamięci frontendu, bez API i zapisu na serwerze lub w pamięci trwałej przeglądarki. Odświeżenie albo zamknięcie strony usuwa stan gościa. Obecnie dostępny jest trening offline, bez nagród online `[D: decyzja właściciela przy F6.2, ADR-018]`.

### 10.2. Lobby i wyzwania

**R-LOB-01. Miejsca na mapie** `[Ź:S2]`: Arena (wyzwania), Trening (boty), Zamek (turnieje), Portal (wyprawy), Bazar (wymiana), Klany (pokój klanowy), Sklep (zestawy kart), Wieża (biblioteka czarów i wysypisko).

**R-LOB-02.** Pokój pokazuje listę obecnych graczy: nazwę, tytuł, aktywną postać i poziom. Listę można sortować według rankingu, poziomu i nazwy. Pokój ma też czat i listę ostatnich pojedynków `[Ź:S1,S2]`.

**R-LOB-03. Filtr wyzwań** każdy gracz ustawia na: wszystkie, tylko od postaci ±2 poziomy albo żadne. Wyzwania, które przejdą filtr, i tak trzeba zaakceptować ręcznie `[Ź:S1,S2,S3]`.

**R-LOB-04.** Wyzwany widzi kartę postaci wyzywającego, jego ranking i pozycję, nazwę, tytuł, klan, statystyki i opis. Na decyzję ma ok. **30 s** `[Ź:S2]` (`challenge.timeout_s`).

**R-LOB-05.** Po wysłaniu lub przyjęciu wyzwania żadna ze stron nie może zmienić postaci ani talii aż do końca pojedynku `[Ź:S1]`.

**R-LOB-06.** Gracz może jednocześnie mieć najwyżej jedno wysłane wyzwanie i brać udział w najwyżej jednym pojedynku `[Z-CHL]`.

### 10.3. Komunikacja

**R-KOM-01.** Wiadomości w pokoju widzą wszyscy obecni w tym pokoju `[Ź:S1,S2]`.

**R-KOM-02. Komendy czatu** `[Ź:S2]`:

| Komenda | Działanie |
|---|---|
| `/t gracz, tekst` | wiadomość prywatna |
| `/r tekst` | odpowiedź ostatniemu rozmówcy |
| `/c tekst` | wiadomość do klanu |
| `/f add|del gracz`, `/f list [all]` | lista znajomych |
| `/i add|del gracz`, `/i list` | lista ignorowanych |
| `/afk` | przełącza status nieobecności |
| `/w gracz` | informacje o graczu |
| `/help` | lista komend |

**R-KOM-03.** Filtr kanałów czatu: wszystkie, klanowe i prywatne, tylko prywatne `[Ź:S2]`.

**R-KOM-04.** Poczta: wiadomości do graczy niezalogowanych `[Ź:S2]`.

### 10.4. Klany

**R-KLA-01.** Klany mogą zakładać tylko konta zarejestrowane `[Ź:S2]`. Założenie: założyciel nadaje nazwę, a klan jest „w fazie zakładania”, dopóki nie zgłoszą się do niego 4 inne osoby; wtedy staje się założony i trafia do rankingu `[Ź:S2]` (`clan.min_members = 5`). Nazwy podlegają regulaminowi `[Ź:S2]`.

**R-KLA-02.** Zgłoszenie do klanu akceptuje mistrz albo v-mistrz `[Ź:S2]`.

**R-KLA-03. Role** `[Ź:S2]`:
- Mistrz — może wszystko;
- V-Mistrz — wszystko poza operacjami na mistrzach;
- Skryba — edycja opisu i wiadomości dnia;
- pozostałe role są tytularne.

**R-KLA-04.** Klan ma własny pokój, niewidoczny dla osób spoza klanu. Ma też specjalne turnieje i ranking `[Ź:S2]`. Profil klanu: nazwa, członkowie z rolami, strona www, e-mail, opis, wiadomość dnia (pokazywana przy wejściu do pokoju), suma Dukatów członków i ranking `[Ź:S2]`.

**R-KLA-05.** Członkostwo w klanie nie wpływa na zasady pojedynków `[Ź:S2]`. Gracz należy do najwyżej jednego klanu `[W]`.

---

## 11. Ekonomia

### 11.1. Dukaty

**R-EKO-01.** Dukaty to waluta gry. W naszej wersji **zdobywa się je wyłącznie w grze** `[D]` (ADR-011). Wartości `[Z-EKO]` do kalibracji:

| Źródło | Domyślnie |
|---|---|
| Nowe konto (zarejestrowane) | 40 (= jeden zestaw podstawowy) |
| Pierwsza wygrana dnia na arenie | +10 |
| Wygrana na arenie | +2 (limit 10 dziennie) |
| Ukończenie wyprawy: Spacer / Wyprawa / Męka | +5 / +10 / +20, plus losowanie z R-WYP-05 |
| Turniej | pula wpisowego i nagrody systemowe |

| Wydatek | Domyślnie |
|---|---|
| Zestaw podstawowy | 40 `[Ź:S2]` |
| Zestawy jednej rzadkości | rozdz. 11.2 |
| Wpisowe do turnieju | ustala twórca |

### 11.2. Zestawy kart (Sklep)

**R-SKL-01.** Zestaw podstawowy: 12 kart (1 złota, 3 srebrne, 8 brązowych) `[Ź:S1,S2,S4]`. Karty losuje się z puli kart dostępnych w sprzedaży, równomiernie w obrębie rzadkości `[Z-BST]`.

**R-SKL-02.** Zestawy złoty, srebrny i brązowy zawierają tylko karty danej rzadkości `[Ź:S2]`. Liczba kart i ceny `[Z-BST]`: brązowy 8 kart za 30, srebrny 4 karty za 40, złoty 2 karty za 60 Dukatów.

**R-SKL-03.** Zakup jest ostateczny: nieotwartego zestawu nie można odsprzedać ani wymienić `[Ź:S2,S4]`. Zestaw trafia na listę zestawów gracza. Po otwarciu karty dodają się do kolekcji `[Ź:S2]`.

**R-SKL-04.** Szanse zawartości zestawów są jawne w interfejsie `[D]`.

### 11.3. Wysypisko (Wieża)

**R-WYS-01.** Gracz może wyrzucić kartę z kolekcji w zamian za punkty: brązowa 5, srebrna 25, złota 100 `[Ź:S4,S6]`. Operacja jest nieodwracalna.

**R-WYS-02.** Za każde 1000 punktów gracz dostaje losowy szablon spośród 3 postaci specjalnych, za każdym razem inny. 3000 punktów daje wszystkie trzy `[Ź:S6]`. Gdy gracz ma już wszystkie trzy, zamiast postaci dostaje 40 Dukatów za każde 1000 punktów `[D]`.

**R-WYS-03.** Postacie z wysypiska mają więcej PR: co 2 poziomy, a nie co 5 `[Ź:S6]`.

### 11.4. Wymiana (Bazar)

**R-WYM-01.** Gracz zaprasza innego gracza do wymiany. Po akceptacji obaj przechodzą do okna wymiany `[Ź:S2]`.

**R-WYM-02.** Każdy układa ofertę: karty z kolekcji i Dukaty. Ofertę wysyła się drugiej stronie; wysłanie do niczego nie zobowiązuje `[Ź:S2]`.

**R-WYM-03.** Każdy może zaakceptować aktualny stan. Jakakolwiek zmiana oferty po akceptacji automatycznie wycofuje akceptacje obu stron `[Ź:S1,S2]`.

**R-WYM-04.** Wymiana dochodzi do skutku, gdy obie strony zaakceptują ten sam stan. Jest atomowa i nieodwracalna `[Ź:S1,S2]`. Serwer sprawdza w transakcji, czy obie strony nadal mają zaoferowane karty i Dukaty (`09-baza-danych.md`).

**R-WYM-05.** Wymieniać mogą tylko konta zarejestrowane. Szablonów postaci i nieotwartych zestawów nie można wymieniać `[Ź:S2,S3]`.

---

## 12. Fair play

**R-FAI-01.** „Podkładanie się” (umawianie wyniku, nabijanie PD lub rankingu z innym kontem) jest zakazane i karane `[Ź:S2,S3]`. Wykrywanie: `12-bezpieczenstwo-i-prawo.md`, rozdz. 4.

**R-FAI-02.** Gracz z rolą **mentora** (w oryginale „mag”) ma oznaczenie w lobby i pomaga nowym graczom. Nie ma uprawnień administracyjnych, a przegrana z nim nie daje PD ani rankingu `[Ź:S3]`.

---

## 13. Parametry konfiguracyjne

Wszystkie wartości liczbowe zasad znajdują się w pliku konfiguracji treści (`content/rules.toml`, `03-tresc-gry.md`). Silnik nie zawiera „magicznych liczb”.

| Klucz | Domyślnie | Reguła | Pochodzenie |
|---|---|---|---|
| `duel.rounds` | 5 | R-RUN | Ź |
| `duel.attacks_per_round` | 3 | R-RUN-04 | Ź |
| `duel.focus_per_round` | 4 | R-RUN-01 | Ź |
| `duel.focus_cap` | brak | R-RUN-02 | Ź |
| `duel.change_cost` | 1 | R-RUN-05 | Ź |
| `duel.hand_size` | 5 | R-POJ-04 | Ź |
| `duel.combo_multipliers` | [1, 2, 4] | R-ATK-04 | Ź |
| `duel.block_min_stamina` | 1 | R-ATK-02 | Ź |
| `duel.default_settings` | `highest_stat` | R-POJ-06 | Ź/W, remis Z-DEF |
| `duel.counter_resolution` | `initiative_first` | R-CZA-05 | Z-CNT |
| `duel.move_timeout_after_opponent_s` | 60 | R-RUN-08 | Ź |
| `duel.round_timeout_s` | 90 | R-RUN-08 | Z-TMO |
| `duel.max_consecutive_timeouts` | 3 | R-RUN-09 | Z-TMO |
| `duel.reconnect_grace_s` | 60 | R-KON-05 | Ź |
| `deck.size` | 20 | R-TAL-01 | Ź |
| `deck.max_copies` | 4 | R-TAL-02 | Ź |
| `deck.fill_missing_with_random` | true | R-TAL-05 | Ź |
| `deck.saved_presets_max` | 10 | R-TAL-06 | Z-PRE |
| `challenge.timeout_s` | 30 | R-LOB-04 | Ź |
| `rating.initial` | 1000 | R-RNK-01 | Z-RNK |
| `rating.k` | 10 | R-RNK-02 | Ź |
| `rating.draw_delta` | 0 | R-RNK-03 | Z-RDR |
| `xp.base` / `xp.min` / `xp.max` | 100 / 10 / 300 | R-PD-02 | Z-XP |
| `xp.tier_mult_pct` / `xp.tier_add` | [100, 110, 120, 130] / [0, 0, 3, 6] | R-PD-02 | Z-XP (S10) |
| `progression.max_level` | 20 | R-ROZ-01 | Ź |
| `progression.reincarnation_xp` | 65000 | R-REI-01 | Ź |
| `progression.dev_point_stat` / `_hp` / `_stamina` | 1 / 7 / 7 | R-ROZ-04 | Ź |
| `progression.reincarnation_bonus_dp` | [2, 4, 6] | R-REI-02 | Z-REI |
| `shade.hp_per_level` / `shade.stamina_per_level` | 4 / 4 | R-CIE-03 | Z-CIE |
| `avatars.max_slots` | 10 | R-PUL-01 | Ź |
| `unlock.arena_chance` | 0.005 | R-NAG-01 | Ź/Z-DRP |
| `unlock.newbie_wins` | [5, 15, 30] | R-NAG-01 | Z-DRP |
| `guest.xp_cap` | 3000 | R-KNT-02 | Ź |
| `training.bot_rotation_min` | 5 | R-TRE-02 | Z-BOT |
| `expedition.stages` | {walk: 4, journey: 6, ordeal: 8} | R-WYP-02 | Z-EXP |
| `expedition.reward_odds` | {avatar: 0.40, card: 0.03, ducats: 0.01} | R-WYP-05 | Ź |
| `tournament.fights_per_phase` | 3 | R-TUR-05 | Ź |
| `tournament.points` | {win: 2, draw: 1} | R-TUR-05 | Ź |
| `tournament.first_fight_delay_s` / `next_fight_delay_s` | 20 / 15 | R-TUR-06 | Ź |
| `tournament.tiebreak` | `random` | R-TUR-05 | Ź |
| `clan.min_members` | 5 | R-KLA-01 | Ź |
| `junkyard.points` | {bronze: 5, silver: 25, gold: 100} | R-WYS-01 | Ź |
| `junkyard.threshold` | 1000 | R-WYS-02 | Ź |
| `shop.basic` | {cards: 12, gold: 1, silver: 3, bronze: 8, price: 40} | R-SKL-01 | Ź |
| `economy.*` | tabela w rozdz. 11.1 | R-EKO-01 | Z-EKO |

## 14. Przykład rundy

Postać A ma inicjatywę: AF 6, OF 1, AM 3, OM 2, Ż 60, W 50. Postać B: AF 4, OF 2, AM 5, OM 1, Ż 55, W 45.

**Ustawienia domyślne (R-POJ-06):**
- A: ataki F F F (AF 6 > AM 3), obrony M M M (OM 2 > OF 1);
- B: ataki M M M (AM 5 > AF 4), obrony F F F (OF 2 > OM 1).

**Decyzje w rundzie 1** (każdy ma K = 4):

| | Ataki | Obrony | Zmiany | Karta | K po rundzie |
|---|---|---|---|---|---|
| A | F **M** F | M **F** M | 2 | — | 4 − 2 = 2 |
| B | M M **F** | F **M** F | 2 | czar „+2 do AM” (K 1, W 4) | 4 − 2 − 1 = 1 |

**Faza czarów:** A nie rzuca nic. B rzuca czar: AM 5 → 7, W 45 → 41.

**Faza ataków:**

| # | Atak | Obrona celu | Wynik | Seria | Obrażenia | Skutek |
|---|---|---|---|---|---|---|
| A1 | F (6) | B: F | blok | A: 0 | — | — |
| B1 | M (7) | A: M | blok | B: 0 | — | — |
| A2 | M (3) | B: M | blok | A: 0 | — | — |
| B2 | M (7) | A: F | trafienie | B: 1 (×1) | 7 − OM_A 2 = 5 | Ż_A 60 → 55, W_B 41 → 34 |
| A3 | F (6) | B: F | blok | A: 0 | — | — |
| B3 | F (4) | A: M | trafienie | B: 2 (×2) | 4×2 − OF_A 1 = 7 | Ż_A 55 → 48, W_B 34 → 30 |

**Koniec rundy:**
- A: Ż 48 (80%), W 50, K 2;
- B: Ż 55 (100%), W 30, K 1. B dobiera kartę w miejsce rzuconej.

Gdyby A miał W = 0 przed atakiem B1, atak B1 trafiłby mimo obrony M (R-ATK-03), a B2 dostałby już mnożnik ×2.

## 15. Rejestr założeń

| ID | Założenie | Uzasadnienie | Weryfikacja |
|---|---|---|---|
| Z-DEF | Remis statystyk przy ustawieniach domyślnych → F | prosta, deterministyczna reguła | brak danych; neutralne dla balansu |
| Z-SPW | Rzucenie wymaga W ≥ koszt; gdy przy rozpatrzeniu nie da się zapłacić, czar przepada bez kosztów | nie można wydać nieposiadanych punktów | test scenariuszowy |
| Z-DSC | Odrzucenie karty nic nie kosztuje | S2 nie wspomina kosztu | — |
| Z-TMO | Limit 90 s na całą rundę, 3 kolejne braki ruchu = walkower | ochrona przed blokowaniem gry | telemetria czasu ruchów |
| Z-NEG | Ż wewnętrznie może być ujemne | upraszcza R-KON-03 | — |
| Z-EMP | Pusta talia → brak dobrania | nie występuje przy 20 kartach | — |
| Z-PRM | Trwałe zmiany statystyk z czarów zwykłych nie są umagicznieniami | rozróżnienie w S8 | — |
| Z-INT | Interwencja: limit na nazwę karty | najprostsza interpretacja S2 | — |
| Z-CNT | Przeciwzaklęcia w kolejności inicjatywy | spójne z R-POJ-05 | — |
| Z-CND | Niespełniony warunek: koszty pobrane, brak efektu | spójne z S3 („czar nie zadziałał”) | — |
| Z-SPC | Redukcja 1/3 z zaokrągleniem `(2k+1) div 3` | korzystne dla gracza, deterministyczne | symulacje balansu |
| Z-IWN | Warunkowa wygrana obu stron → remis | spójne z R-KON-01 | — |
| Z-FRF | Ucieczka = zwykła przegrana | — | — |
| Z-RNK | Ranking startowy 1000 | zakres tytułu „student esencji” | — |
| Z-RDR | Remis nie zmienia rankingu | brak danych | — |
| Z-RMN | Ranking ≥ 1 | ochrona przed dzieleniem przez 0 | — |
| Z-KLR | Ranking klanu = średnia 10 najlepszych | odporność na liczbę członków | — |
| Z-XP | Wzór PD | oryginał nieznany | symulacje tempa progresji, telemetria |
| Z-DRP | Szanse i gwarancje postaci z areny | przybliżenie danych z S3 | telemetria |
| Z-REI | Premia PR za reinkarnację 2/4/6 | „liczba rośnie” (S1/S2), skala z S10 | symulacje |
| Z-GFT | Dar = darmowe umagicznienie od startu | S2: „umiejętność dająca nowe możliwości w walce” | projekt treści |
| Z-CIE | Cień +4 Ż, +4 W na poziom | brak danych | symulacje |
| Z-PRE | 10 zapisanych talii | — | — |
| Z-TRM | Automatyczne przycinanie talii | spójność danych | — |
| Z-BOT | Rotacja botów co 5 min | S2: „co kilka minut” | — |
| Z-EXP | Liczba etapów, nagrody Dukatowe, częściowe PD, brak rankingu w wyprawach | brak danych | symulacje, telemetria |
| Z-TUR | Wolne losy, walkower przy nieobecności lub opuszczeniu | brak danych | — |
| Z-CHL | Jedno wyzwanie i jeden pojedynek naraz | prostota i spójność stanu | — |
| Z-EKO | Źródła i ujścia Dukatów | brak płatności (D) | monitoring inflacji (`08-serwer.md`) |
| Z-BST | Zawartość i ceny zestawów jednej rzadkości | S2: „w ilości wymienionej na liście” | balans ekonomii |
