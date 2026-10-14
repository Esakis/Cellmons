# 18. Słownik pojęć

Pojęcia gry w trzech wersjach: polska nazwa (używana w dokumentacji i interfejsie), angielska (interfejs EN) i identyfikator w kodzie. Kolumna „Oryginał” pokazuje nazwę z gry Cellmons tam, gdzie się różni albo jest charakterystyczna.

## 1. Rozgrywka

| Polski | Angielski | Kod | Oryginał / uwagi |
|---|---|---|---|
| Pojedynek | Duel | `Duel` | — |
| Runda | Round | `round` | maks. 5 |
| Atak / obrona fizyczna (AF / OF) | Physical attack / defense | `phys_atk`, `phys_def` | — |
| Atak / obrona magiczna (AM / OM) | Magical attack / defense | `mag_atk`, `mag_def` | — |
| Punkty życia (Ż) | Life | `hp` | — |
| Punkty wytrzymałości (W) | Stamina | `stamina` | potocznie „wyt” |
| Punkty koncentracji (K) | Focus | `focus` | — |
| Ustawienia ataków i obron | Attack and defense settings | `attacks`, `defenses` | — |
| Slot | Slot | `slot` | jeden z 3 ataków lub 3 obron |
| Blok | Block | `AttackBlocked` | — |
| Trafienie | Hit | `AttackHit` | — |
| Combo (seria trafień) | Combo | `combo` | ×1, ×2, ×4 |
| Inicjatywa | Initiative | `initiative` | losowana raz na pojedynek |
| Ucieczka | Forfeit | `Reason::Forfeit` | — |
| Walkower | Walkover | `Reason::Timeout` / `Forfeit` | — |
| Remis | Draw | `Outcome::Draw` | — |
| Procent życia | Life percentage | — | rozstrzyga po 5 rundach |
| Ruch | Move | `Move` | ustawienia + akcja kartą |
| Ziarno losowania | Seed | `seed` | — |
| Zobowiązanie do ziarna | Seed commitment | `seed_commitment` | skrót BLAKE3 ziarna |
| Powtórka | Replay | `ReplayRecord` | — |

## 2. Karty

| Polski | Angielski | Kod | Oryginał / uwagi |
|---|---|---|---|
| Karta czaru, czar | Spell card | `CardDef` | — |
| Talia (zestaw czarów) | Deck | `deck` | 20 kart, maks. 4 kopie |
| Ręka | Hand | `hand` | 5 kart |
| Kolekcja | Collection | `collection` | w oryginale „Biblioteka Czarów” |
| Rzadkość: brązowa / srebrna / złota | Bronze / Silver / Gold | `Rarity` | — |
| Wymagania (szkoły) | Requirements | `requires` | liczba symboli szkoły |
| Umagicznienie | Enchantment | `Enchantment` | działa co rundę do końca walki |
| Interwencja | Intervention | `Keyword::Intervention` | raz na pojedynek |
| Przeciwzaklęcie | Counterspell | `Keyword::Counter` | — |
| Sprzężenie | Coupling | `Keyword::Coupling` | dopłata za poziom szkoły |
| Rozproszenie | Dispel | `dispel` | usuwa umagicznienia |
| Rodzina efektu | Effect family | `FamilyEffect` | — |
| Efekt specjalny | Custom effect | `EffectSpec::Custom` | — |
| Zestaw kart (booster) | Booster pack | `booster` | podstawowy: 12 kart |
| Dodatek / edycja | Set / edition | `set` | — |

## 3. Postacie i rozwój

| Polski | Angielski | Kod | Oryginał / uwagi |
|---|---|---|---|
| Postać | Avatar | `Avatar` | w oryginale też „Cellmon” |
| Szablon postaci | Avatar template | `template_id` | „karta postaci” |
| Pula postaci | Avatar roster | `avatars` | maks. 10 |
| Aktywna postać | Active avatar | `is_active` | — |
| Szkoła (Esencji) | School | `School` | Walka, Kapłaństwo, Elementalizm, Magia, Nekromancja, Technologia |
| Biegłość: Nowicjusz / Adept / Mistrz / Arcymistrz | Novice / Adept / Master / Grandmaster | poziom 1–4 | — |
| Punkty doświadczenia (PD) | Experience (XP) | `xp` | — |
| Poziom | Level | `level` | 1–20 |
| Punkty rozwoju (PR) | Development points | `dev_points` | +1 statystyka lub +7 Ż/W |
| Reinkarnacja | Reincarnation | `tier` | po poz. 20 i 65 000 PD |
| Poziom reinkarnacji: Adept / Mistrz / Ascent | Adept / Master / Ascendant | `tier` 1–3 | „Ascent” w oryginale |
| Droga | Path | `path_id` | wybór przy 1. reinkarnacji |
| Specjalizacja | Specialization | `specialization` | wybór przy 2. reinkarnacji |
| Dar | Gift | `gift_id` | wybór przy 3. reinkarnacji |
| Cień | Shade | `is_shade` | z poświęcenia postaci Ascent |

## 4. Świat gry i tryby

| Polski | Angielski | Kod | Oryginał / uwagi |
|---|---|---|---|
| Mapa | Map | `map` | — |
| Arena | Arena | `room: arena` | pojedynki rankingowe |
| Sala treningowa | Training room | `room: training` | boty „AI” |
| Zamek | Castle | `room: castle` | turnieje |
| Portal | Portal | `room: portal` | w oryginale „Tajemniczy Portal”, wyprawy |
| Bazar | Bazaar | `room: bazaar` | wymiana |
| Sklep | Shop | `shop` | — |
| Wieża | Tower | `tower` | „Wieża Maga”: biblioteka i wysypisko |
| Wysypisko | Junkyard | `junkyard` | — |
| Wyprawa: Spacer / Wyprawa / Męka | Expedition: Walk / Journey / Ordeal | `difficulty` | — |
| Turniej, faza | Tournament, phase | `tournament`, `phase` | — |
| Klan, pokój klanowy | Clan, clan room | `clan` | w oryginale „namiot klanowy” |
| Mistrz / V-Mistrz / Skryba | Master / Vice-master / Scribe | `role` | role w klanie |
| Mentor | Mentor | `role: mentor` | w oryginale „mag” |
| Dukaty | Ducats | `ducats` | waluta gry |
| Ranking, tytuł | Rating, title | `rating` | — |
| Konto gościa | Guest account | `email_verified_at IS NULL` | w oryginale „konto niezarejestrowane” |
| Podkładanie się | Win trading | `abuse_signals` | zakazane |

## 5. Aplikacja mobilna

| Pojęcie | Znaczenie w projekcie |
|---|---|
| APK | plik instalacyjny aplikacji Android; u nas do pobrania ze strony |
| AAB (Android App Bundle) | format publikacji w Google Play; sklep tworzy z niego APK dla konkretnych urządzeń |
| Capacitor | natywna powłoka uruchamiająca klienta web w Android System WebView (ADR-014) |
| Powłoka natywna | część aplikacji w kodzie Androida (Capacitor + wtyczki); wydawana rzadko |
| Paczka web | podpisany zestaw JS, CSS, WASM, treści i grafik, aktualizowany bez nowego APK (`19-aplikacja-android.md`, rozdz. 5) |
| Token aplikacji | identyfikator sesji aplikacji przechowywany w magazynie chronionym przez Android Keystore (ADR-015) |
| Bilet WebSocket | jednorazowe, krótkotrwałe pozwolenie na połączenie WebSocket z aplikacji |
| Klucz podpisu aplikacji / klucz przesyłania | klucz, którym podpisane są instalowane aplikacje / klucz do wysyłania wydań do Google Play |
| Play App Signing | usługa Google Play przechowująca klucz podpisu aplikacji |
| D-U-N-S | numer identyfikacyjny firmy wymagany przy koncie organizacji w Google Play |
| Test zamknięty | ścieżka Google Play dla ograniczonej grupy testerów przed publikacją |
| „Bezpieczeństwo danych” | formularz Google Play opisujący zbierane dane |

## 6. Pojęcia techniczne

| Pojęcie | Znaczenie w projekcie |
|---|---|
| Silnik | biblioteka Rust z zasadami gry, bez wejścia/wyjścia (`06-silnik-gry.md`) |
| Aktor | zadanie Tokio z własnym stanem i skrzynką wiadomości (`08-serwer.md`) |
| `PlayerView` | stan pojedynku widoczny dla jednego gracza, bez ukrytych informacji |
| `content_version` | skrót wersji treści gry (`03-tresc-gry.md`, rozdz. 8.4) |
| Dziennik rund | zapis ruchów trwającego pojedynku w bazie, pozwala odtworzyć walkę po restarcie |
| Księga (ledger) | zapis podwójny każdej zmiany Dukatów (`09-baza-danych.md`, rozdz. 3.5) |
| Wygaszanie (drain) | łagodne zamykanie instancji serwera przy wdrożeniu (`08-serwer.md`, rozdz. 9) |
| Blue/green | dwa sloty serwera, między którymi przełącza się ruch przy wdrożeniu |
| PITR | odtworzenie bazy do dowolnego punktu w czasie |
| Reguła `R-…` | identyfikator reguły w specyfikacji zasad (`02-zasady-gry.md`) |
| Założenie `Z-…` | decyzja projektowa tam, gdzie źródła milczą (rejestr w `02-zasady-gry.md`, rozdz. 15) |
| `S1`–`S12` | kody źródeł (`01-badania-i-zrodla.md`, rozdz. 4) |
| TR-01 | test referencyjny na podstawie zarchiwizowanej walki (`14-testy-i-jakosc.md`, rozdz. 3) |

## 7. Nazwa i znak towarowy

| Pojęcie | Znaczenie w projekcie |
|---|---|
| Znak słowny | znak chroniący samą nazwę, bez grafiki; zgłaszamy SPELLGAMBIT (`20-nazwa-i-znak-towarowy.md`) |
| Znak unijny | znak rejestrowany w EUIPO, chroniony we wszystkich państwach UE |
| EUIPO / UPRP / USPTO | urząd unijny / polski / amerykański rejestrujący znaki towarowe |
| TMview | wspólna wyszukiwarka znaków urzędów UE, WIPO i wielu innych krajów |
| TMclass, baza zharmonizowana | baza nazw towarów i usług, które EUIPO przyjmuje bez zastrzeżeń (`20-nazwa-i-znak-towarowy.md`, rozdz. 5) |
| Klasa (klasyfikacja nicejska) | grupa towarów lub usług; dla gier: 9 (oprogramowanie), 41 (usługi gier), 28 (gry fizyczne) |
| Sprzeciw | wniosek właściciela wcześniejszego znaku o odmowę rejestracji; w EUIPO 3 miesiące od publikacji zgłoszenia |
| Pierwszeństwo | prawo do daty pierwszego zgłoszenia przy zgłoszeniach w innych krajach w ciągu 6 miesięcy |
| Fast Track | szybsza ścieżka badania w EUIPO przy wykazie z bazy zharmonizowanej i opłacie przy zgłoszeniu |
| SME Fund | unijny program zwrotu części opłat za znaki dla małych i średnich firm |
| System madrycki | zgłoszenie znaku w wielu krajach przez WIPO na podstawie znaku krajowego lub unijnego |
