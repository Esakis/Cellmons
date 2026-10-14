# 01. Badania i źródła

Ten dokument zbiera wszystko, co udało się ustalić o oryginalnej grze Cellmons: historię, wersje, źródła i ich wiarygodność. Opisuje też weryfikację zasad na zachowanych zapisach walk. Na tej podstawie powstała specyfikacja zasad (`02-zasady-gry.md`) i treści (`03-tresc-gry.md`).

## 1. Czym był Cellmons

Cellmons („Cellular Monsters”) to polska gra sieciowa łącząca **kolekcjonerską grę karcianą (CCG)** z **rozwojem postaci jak w RPG**. Gracz miał pulę do 10 postaci („Cellmonów”), budował dla aktywnej postaci talię 20 czarów i staczał pojedynki 1 na 1 z innymi graczami przez centralny serwer. Rdzeniem rozgrywki była zgadywanka w stylu „papier–kamień–nożyce”: w każdej z 5 rund obaj gracze w ukryciu ustawiali 3 ataki i 3 obrony (fizyczne lub magiczne) i wybierali czar. Wygrywał zwykle ten, kto trafniej przewidział zachowanie przeciwnika. Zwycięstwa dawały doświadczenie (rozwój postaci) i punkty rankingu.

- **Producent:** Pogram, marka firmy Toolzz & Toyzzz (założonej w 2002 r.).
- **Platformy:** klient Windows oraz telefony z Java ME (Siemens S55, Motorola T720/T720i/T722i). Pojedynki między platformami były możliwe.
- **Model biznesowy (wersja klasyczna):** darmowy test (do 3000 PD), potem waluta Dukaty kupowana za pieniądze (SMS, pakiety). Zestaw podstawowy przedłużał ważność konta o 30 dni.
- **Wznowienie 2010:** nowa wersja „Cellmons — New free CCG-MMORPG game” (PL/EN), darmowa z mikropłatnościami (Gold), z dodatkową mechaniką Eteru.

## 2. Oś czasu

| Data | Wydarzenie | Źródło |
|---|---|---|
| 2002 | Powstaje Toolzz & Toyzzz (Pogram) | strona „O nas” cellmons.com (2003) |
| maj–lip 2003 | Zapowiedzi i nabór do beta testów (PC: Pentium MMX, 64 MB RAM, Windows 98+; J2ME: ok. 80 kB, GPRS) | Polygamia, GRYOnline, GSMOnline |
| 15.10.2004 | „Wkrótce startujemy” — zapowiedź „pierwszej w Polsce gry massivemultiplayer” | aktualności cellmons.tenbit.pl |
| 22.10.2004 | Start serwera gry na portalu tenbit.pl | aktualności cellmons.tenbit.pl, GRYOnline (28.10.2004) |
| 26.11.2004 | Pełna wersja płatna, darmowy test do 3000 PD | Polter |
| 2005–2008 | Dodatki: Cellmons Core, Zemsta Technologa, Czas Oczekiwania, Trzeci Sztorm, Magia Utopców. Fanowski serwis Cellmons Guru (od 2005), liga klanowa | Cellmons Guru |
| 2007 | Nowe ilustracje kart, próby wznowienia gry | DeviantArt (Com2odo) |
| 26.03.2010 | Otwarta beta nowej wersji na cellmons.com (PL/EN), nowe zasady (Eter, Gold) | cellmons.com, Cellmons Guru |
| 2010–2013 | Ostatnie zarchiwizowane zapisy walk i posty na forum (sierpień 2013) | Internet Archive |
| 2016 | Domena cellmons.com zajęta przez niepowiązaną stronę | Internet Archive |

> FAQ Cellmons Guru wspomina „pracowników Onetu” i kody promocyjne z magazynu „Click!”. Sugeruje to współpracę z innymi wydawcami w latach 2005–2007, ale nie zostało to potwierdzone.

## 3. Wersje gry — co odtwarzamy

Decyzja projektowa: **odtwarzamy wersję klasyczną (2004–2009)**. Wersja 2010 służy jako uzupełniające źródło danych (koszty kart, statystyki postaci, zapisy walk), bez mechanik, których klasyka nie miała.

| Aspekt | Klasyczna 2004–2009 (cel) | Wznowienie 2010–2013 (tylko referencja) |
|---|---|---|
| Waluta | Dukaty | Gold |
| Eter (−9…+9, karty natychmiastowej wygranej) | **brak** | tak |
| Szkoły | 5 na starcie + Technologia (dodatek „Zemsta Technologa”) = 6 | 6 |
| Talia | 20 kart, max 4 kopie (fani w 2007–2008 zapowiadali 30) | jak klasyczna |
| Konto | test do 3000 PD (później 10 000), potem płatne konto | darmowe |
| Remis fazy turnieju | losowanie | wygrywa mniej doświadczona postać, potem losowanie |
| Ranking | wzór z FAQ (2006) | ten sam wzór, potwierdzony w zapisach walk |
| Platformy | Windows + J2ME | Windows |

W naszej wersji **nie ma płatności za prawdziwe pieniądze** na starcie (ADR-011). Dukaty zdobywa się w grze, a ograniczenia konta gościa zostają jako zabezpieczenie przed nadużyciami (patrz `02-zasady-gry.md`, rozdz. 10).

## 4. Katalog źródeł

Kody źródeł (S1–S12) występują w specyfikacji zasad przy każdej regule.

| Kod | Źródło | Okres | Zawartość | Wiarygodność |
|---|---|---|---|---|
| S1 | Oficjalna instrukcja, cellmons.com (`o_grze_2a.asp`) | 2003–2004 | postacie, statystyki, reinkarnacja, czary (talia 20, losowane 5), pojedynek (5 rund, 3 ataki + 3 obrony, koncentracja, combo, inicjatywa), lobby, turnieje, wymiana, klient J2ME | wysoka (oficjalna) |
| S2 | Pomoc w grze, cellmons.tenbit.pl (`help/*.php`) | 2004–2006 | zasady walki, **przykładowa walka krok po kroku**, ekrany, wyprawy, turnieje (punkty, faza), turnieje klanowe, wymiana, sklep, ważność konta, cennik, lobby i komendy czatu, klany i role, rozłączenia | wysoka (oficjalna) |
| S3 | FAQ Cellmons Guru (`cellmons_faq.php`) | 2006–2007 | combo, poziomy biegłości szkół, specjalizacje, cienie, **wzór rankingu**, tytuły rankingowe, wyprawy (szanse nagród), zdobywanie postaci, sprzężenia, konto niezarejestrowane | wysoka (fani, zgodne z S1/S2) |
| S4 | Słownik i instrukcja, cellmons.com (`manual/*`) | 2010 | combo, koncentracja, interwencja, ranking (suma zerowa), reinkarnacja, boostery, wysypisko, Eter | wysoka (oficjalna, wersja 2010) |
| S5 | Zapisy walk, cellmons.com (`fights/*`) | 2010–2013 | dla każdej rundy: ataki, obrony, czar, życie, wytrzymałość, koncentracja, Eter; zmiana rankingu | bardzo wysoka (surowe dane z serwera) |
| S6 | Cellmons Guru: rozwój postaci, reinkarnacje, cienie, wysypisko, zestawy początkowe | 2006–2008 | tabele rozwoju poz. 1–20, drogi, specjalizacje, dary, zasady cieni | wysoka (spisane przez graczy) |
| S7 | Lista kart, cellmons.tenbit.pl (`kartylista.php`) | 2005–2006 | 338 kart: nazwa, rzadkość, wymagania szkół | bardzo wysoka |
| S8 | Lista czarów i postaci, cellmons.com (`effects`, `avatars`, `monster/*`, `effect_detail/*`) | 2010 | koszty 186 klasycznych kart, statystyki 31 postaci, opisy ok. 20 kart | wysoka (wersja 2010) |
| S9 | Manuskrypt „taktyce na polu bitwy” (Cellmons Guru) | 2007 | archetypy talii i kluczowe karty (kategorie efektów) | średnia (opinie gracza) |
| S10 | Regulamin Ligi Cellmons (Cellmons Guru) | ok. 2006 | wartościowanie postaci po reinkarnacji (Adept ×1,1; Mistrz ×1,2 + 3; Ascent ×1,3 + 6) | średnia (umowa graczy) |
| S11 | Artykuły: GRYOnline, GSMOnline, Polygamia, Polter, Sztab.com | 2003–2004 | opis ogólny, platformy, wymagania, model płatności | wysoka (fakty), niska (szczegóły zasad) |
| S12 | Bazy gier: Lutris, Kotaku, Glitchwave, IGDB | — | opis ogólny, daty | średnia |

### Linki

- S1: <https://web.archive.org/web/20040219015330/http://www.cellmons.com/o_grze_2a.asp>
- S2: <https://web.archive.org/web/20050116132329/http://cellmons.tenbit.pl/help/zasady.php>, przykładowa walka: <https://web.archive.org/web/20041106105954/http://cellmons.tenbit.pl/help/przykladowawalka.php>, wyprawy: <https://web.archive.org/web/20050427100153/http://cellmons.tenbit.pl/help/wyprawy.php>
- S3: <https://web.archive.org/web/20060316152209/http://www.cellmons.waw.pl/cellmons_faq.php>
- S4: <https://web.archive.org/web/20100401140353/http://www.cellmons.com/manual/dictionary>
- S5: przykład <https://web.archive.org/web/2010/http://www.cellmons.com/fights/26519>
- S6: <http://www.cellmons.waw.pl/> (strona działa, ale baza danych serwisu jest uszkodzona; pełne dane są w Internet Archive)
- S7: <https://web.archive.org/web/2006/http://cellmons.tenbit.pl/kartylista.php>
- S8: <https://web.archive.org/web/20100330064036/http://www.cellmons.com/avatars>, <https://web.archive.org/web/20100330070612/http://www.cellmons.com/pl/effects>
- S11: [GRYOnline 2003](https://www.gry-online.pl/newsroom/cellmons-czyli-komorkowe-potwory/z1245f), [GRYOnline 2004](https://www.gry-online.pl/newsroom/cellmons-pierwszy-polska-gra-strategiczna-mmo-rusza-do-boju/z33f73), [GSMOnline](https://gsmonline.pl/artykuly/cellmons-polska-gra-dla-komputerow-i-telefonow-komorkowych), [Polygamia](https://polygamia.pl/beta-testy-gry-cellmons,6572108681594497a), [Polter](https://polter.pl/karcianki/Karcianka-w-tenbit-pl-w4279), [Sztab.com](https://www.sztab.com/g-cellmons/)
- S12: [Lutris](https://lutris.net/games/cellmons/), [Kotaku](https://kotaku.com/games/cellmons), [Glitchwave](https://glitchwave.com/game/cellmons/)

## 5. Weryfikacja zasad na danych

### 5.1. Wzór rankingu

FAQ (S3) podaje: zwycięzca zyskuje, a przegrany traci tę samą liczbę punktów:

```
Δ = 10 × (R_przegranego / R_zwycięzcy) × (poziom_przegranego / poziom_zwycięzcy)
```

Sprawdzenie na wszystkich zachowanych walkach z niezerową zmianą (S5). R to ranking **przed** walką (nowy ranking minus zmiana):

| Walka | Zwycięzca (R, poz.) | Przegrany (R, poz.) | Wzór | Zapis w logu |
|---|---|---|---|---|
| 26519 | 929, 14 | 967, 11 | 8,18 | ±8 |
| 26520 | 937, 14 | 959, 11 | 8,04 | ±8 |
| 26527 | 951, 11 | 581, 10 | 5,55 | ±5 |
| 248644 | 1228, 7 | 816, 8 | 7,59 | ±7 |
| 266338 | 1062, 7 | 872, 6 | 7,04 | ±7 |
| 314437 | 1240, 14 | 1364, 15 | 11,79 | ±11 |
| 324051 | 1645, 14 | 1210, 14 | 7,36 | ±7 |

Wynik: 7 na 7 zgodnych przy **zaokrągleniu w dół** (11,79 → 11 wyklucza zaokrąglenie do najbliższej liczby).

### 5.2. Model walki — rekonstrukcja walki #26519

Walka z 30.03.2010: P1 to Draco na poz. 11, P2 to Mnich na poz. 14 (P2 wygrał). Log podaje dla każdej rundy ustawienia ataków i obron (F = fizyczny, M = magiczny), rzucony czar oraz stan **po** rundzie: życie (Ż), wytrzymałość (W) i koncentrację (K).

| R | P1 ataki | P1 obrony | P1 czar | P1 Ż/W/K | P2 ataki | P2 obrony | P2 czar | P2 Ż/W/K |
|---|---|---|---|---|---|---|---|---|
| 1 | M F F | M M F | — | 55/66/1 | F F F | M M M | „Mass” | 60/77/2 |
| 2 | F M F | M F F | — | 49/53/2 | F F F | M F F | „Mass” | 45/61/2 |
| 3 | M M M | M F M | — | 37/35/3 | F F F | F M F | „Medicaments” | 56/37/2 |
| 4 | M M M | F M F | Ściana Ognia | 0/10/1 | F F F | F M M | „Overwhelming Attack” | 43/3/1 |

Statystyki Mnicha na poz. 14 z tabeli rozwoju fanów (S6): AF 7, OF 3, AM 7, OM 4, Ż 66, W 87 oraz 3 punkty rozwoju. Przy założeniu, że 1 punkt poszedł na AF (+1), a 2 na W (+2×7), Mnich ma **AF 8, OF 3, AM 7, OM 4, Ż 66, W 101**. Dla Draco z danych wynika: AF 4, AM 9, OF 2, Ż 75, W 74.

Przy regułach z S1/S2 każda obserwowana liczba w rundach 1–3 zgadza się co do punktu:

- **Obrażenia = atak × mnożnik combo − obrona.** Runda 1, P1 → P2: blok, potem (4×1−3) + (4×2−3) = 1 + 5 = 6, czyli Ż 66 → 60 ✔. Runda 2: (4×1−3) + (9×2−4) = 15, czyli 60 → 45 ✔. Pojedyncze trafienia P2 → P1 zawsze za 8−2 = 6 (runda 2: 55 → 49 ✔, runda 3: 2 × 6, 49 → 37 ✔).
- **Seria (combo) zeruje się po bloku:** w rundzie 3 trafienia w slotach 1 i 3 przedzielone blokiem liczą się ×1, a nie ×2 ✔.
- **Koszt wytrzymałości = wartość ataku za każde trafienie** (bez mnożnika, bez kosztu za blok, bez regeneracji). P1: runda 2 4 + 9 = 13 (66 → 53 ✔), runda 3 9 + 9 = 18 (53 → 35 ✔), runda 4 9 + 16 za Ścianę Ognia (koszt W 16 z S8) = 25 (35 → 10 ✔). P2: runda 1 8 + 8 + 8 („Mass”) = 24 (101 → 77 ✔), runda 2 8 + 8 = 16 (77 → 61 ✔).
- **Koncentracja: +4 na rundę, kumuluje się, 1 za każdą zmianę slotu względem poprzedniej rundy, plus koszt czaru.** P1 runda 2: 1 + 4 − 3 zmiany = 2 ✔. Runda 3: 2 + 4 − 3 = 3 ✔. Runda 4: 3 + 4 − 3 zmiany − 3 (Ściana Ognia, K 3 z S8) = 1 ✔. P2 runda 2: 2 + 4 − 2 zmiany − 2 („Mass”) = 2 ✔.
- **Domyślne ustawienia w 1. rundzie = typ wyższej statystyki.** Mnich: AF 8 > AM 7, więc ataki domyślnie F F F; OM 4 > OF 3, więc obrony M M M. 0 zmian, a 4 − 2 („Mass”) = 2 ✔. Draco: AM 9 > AF 4, więc ataki M M M; OM > OF, więc obrony M M M. 3 zmiany, 4 − 3 = 1 ✔. Zgodne z opisem w przykładowej walce (S2).
- **Runda 3, P2:** 56 = 45 + 21 − 10, czyli „Medicaments” leczy o 21 albo do maksimum 66. Obie wersje są spójne z modelem.
- **Punkty rozwoju:** przyjęty podział 3 punktów (+1 AF, +2×7 W) daje maksimum W 101 = 87 + 14. To rozwiązanie spójne ze wszystkimi obserwacjami, nie niezależny dowód. Reguła „+1 do statystyki lub +7 Ż/W” pochodzi z S2/S3.

Runda 4 jest zgodna z modelem pod warunkiem, że „Overwhelming Attack” sprawia, że ataki P2 nie mogą zostać zablokowane (3 trafienia: 6 + 14 + 30 ≥ 37), a Ściana Ognia zadaje 8 obrażeń. Tekstów tych kart nie znamy.

> Rundy 1–3 tej walki to **test referencyjny TR-01** silnika (`14-testy-i-jakosc.md`). Silnik zgodny ze specyfikacją musi odtworzyć wszystkie wartości Ż/W/K z tabeli.

### 5.3. Spójność danych kart i postaci

- Wymagania szkół 338 kart z listy klasycznej (S7) są zgodne z danymi 2010 (S8) dla wszystkich 186 kart obecnych w obu źródłach.
- Rzadkość z S7 (kolor) zgadza się z rzadkością na stronach szczegółów kart 2010.
- Statystyki bazowe postaci 2010 (S8) są zgodne z tabelami rozwoju fanów z 2007 (S6) dla wszystkich postaci, które występują w obu źródłach.

## 6. Czego nie wiemy

| Luka | Skutek | Rozwiązanie |
|---|---|---|
| Teksty efektów ok. 318 z 338 kart | nie da się odtworzyć kart 1:1 | projektujemy efekty sami (i tak wymagane ze względów prawnych — ADR-012). Nazwy kart i koszty z 2010 służą jako wskazówka intencji |
| Wzór PD za walkę | brak tempa progresji | wzór projektowy Z-XP (`02-zasady-gry.md`, rozdz. 7), kalibrowany symulacjami i telemetrią |
| Tabele rozwoju 15 postaci | brak krzywych wzrostu | generator krzywych na wzór 16 znanych tabel (`03-tresc-gry.md`, rozdz. 3) |
| Dokładne działanie przeciwzaklęć przy dwóch przeciwzaklęciach | możliwe spory przy implementacji | założenie Z-CNT, konfigurowalne |
| Premia punktów rozwoju za reinkarnację | brak liczb | założenie Z-REI |
| Liczba etapów wyprawy | brak | założenie Z-EXP |
| Szczegóły darów (np. jak działa dar „Wampiryzm”) | brak | dar = efekt karty o tej nazwie aktywny od początku pojedynku (Z-GFT) |

## 7. Jak powtórzyć badania

1. Indeks archiwum: `https://web.archive.org/cdx/search/cdx?url=<domena>&matchType=domain&output=txt&fl=timestamp,original,mimetype,statuscode&collapse=urlkey`. Domeny: `cellmons.com`, `cellmons.tenbit.pl`, `cellmons.waw.pl`, `cellmons.fora.pl`.
2. Surowe strony: `https://web.archive.org/web/<timestamp>id_/<original>` (bez paska archiwum).
3. Kodowanie: strony z lat 2003–2008 to ISO-8859-2, a z 2010 UTF-8.
4. Ikony wymagań w liście kart: `/i/cellmons/wymagania/<szkoła>/<rzadkość>.gif`, gdzie szkoły to 1 walka, 2 kapłaństwo, 3 elementalizm, 4 nekromancja, 5 magia, 6 technologia; rzadkość: 1 brązowa, 2 srebrna, 3 złota.
5. Zapisy walk 2010: ikony `combat*.png` oznaczają atak/obronę fizyczną, `arcane*.png` magiczną.
