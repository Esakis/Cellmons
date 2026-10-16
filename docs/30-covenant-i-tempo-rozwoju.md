# 30. Covenant i model tempa rozwoju

Katalog ma 198 kart w czterech edycjach. Pierwsza paczka Covenant dodaje 18 złotych czarów wymagających biegłości 4 oraz 15 srebrnych kombinacji wszystkich par szkół na poziomie 2. Każda karta ma tekst PL/EN, osobny rysunek SVG i niezależny scenariusz liczbowy w `crates/engine-content/tests/cards/covenant.rs`. Testy obejmują kolejność efektów, trafienia, nietrafienia, redukcję obrażeń i wygaśnięcie premii po rundzie. Złote czary pasują do wrodzonej biegłości Perfekcji; inne profile potrzebują dodatkowego podniesienia szkoły w walce.

Przegląd darów Relic usunął dwa oczywiste przypadki przewagi dodatkowych efektów przy tych samych kosztach i wymaganiach. Welon kroniki daje teraz 2 K na rundę: wybór względem Kamienia mędrca (1 K i 1 Ż) lub Berła astralnego (1 K i biegłość) ma koszt alternatywny. Aureola odnowy daje 2 Ż i 1 W na koniec rundy, a Żywy eliksir 3 Ż na początku. Ich liczby i teksty pochodzą z danych; nie są odtworzeniem brakujących opisów archiwalnych.

Po paczce sprawdzono scenariusze wszystkich 198 kart, kompilator, clippy, native/WASM, typy, Vitest i przeglądarkę desktop/Pixel 5. Galeria `art/covenant-review.html` zawiera 33 przejrzane ilustracje, które oczekują na odbiór właściciela. Budżet przed korektą dwóch darów: offline 195,7/200 KiB, start JS 52,7/70, walka JS 14,6/40, WASM 65,1/90, ilustracje kart 31,1/80, moduły online 43,0/45.

Audyt L1 rozegrał 180 000 pojedynków na 25 szablonach bez cieni, dla poziomów 1/10/20, z losowanymi legalnymi taliami i zamianą stron. Nie wydaje PR ani nie wybiera dróg, więc jest badaniem pomocniczym, a nie spełnieniem kryterium balansu L2 pełnych konfiguracji. Po korekcie darów skrajne wyniki nadal wynoszą:

| Poziom | Najniższy wynik | Najwyższy wynik |
|---|---|---|
| 1 | Rycerz żużlu 16,1% | Wyrocznia przypływu 73,0% |
| 10 | Rycerz żużlu 25,2% | Wyrocznia przypływu 65,8% |
| 20 | Mag 25,7% | Jedwabne ostrze 69,8% |

Zmiany dwóch darów nie przesunęły żadnego agregatu o 3 p.p. Nie rozwiązują odchyleń postaci; pozostają one jawnie otwarte do F10.5. Logi: `target/f10-covenant-*`; pary wyników po korekcie: `target/roster-balance.csv`.

## Model tempa

`cargo run -p server --example progression_audit --locked -- --samples 100` używa tej samej funkcji PD co rozliczenie walki. Opcje: `--base`, `--win-bps`, `--draw-bps`, `--seconds`, `--daily-minutes`, `--samples`. Test dokładnych wygranych i remisów sprawdza progi oraz koszt wszystkich ofiar.

Przyjęto zweryfikowane konto, 50% wygranych, brak remisów, równe poziomy i efektywne stopnie przeciwników, 90 s na walkę i 30 min gry dziennie. To jawne założenia, bez pomiarów zachowania graczy. Przybliżone mediany 100 powtarzalnych karier:

| Kamień | Baza PD 100 (obecna) | Baza PD 150 | Baza PD 300 |
|---|---|---|---|
| Pierwszy poziom 20 | 50 dni | 34 dni | 17 dni |
| Pierwsza reinkarnacja | 65 dni | 44 dni | 22 dni |
| Ascent na poziomie 1 | 195 dni | 131 dni | 65 dni |
| Pierwszy cień | 260 dni | 174 dni | 87 dni |
| Cień Ascent wraz ze wszystkimi ofiarami | 845 dni | 564 dni | 283 dni |

Jeśli cień walczy z postaciami bez reinkarnacji na tym samym poziomie, ostatni kamień rośnie odpowiednio do około 1025/684/342 dni. Wycena cienia jak Ascent działa na każdym jego stopniu. Ofiary wymagają łącznie sześciu dodatkowych cykli zwykłych postaci; utworzenie cienia wymaga czterech cykli źródłowego Ascent.

Zachowano obecną bazę 100 do wyboru docelowego tempa. Wariant 150 utrzymuje możliwość większej nagrody za trudniejszego przeciwnika przy limicie 300 PD. Wariant 300 osiąga ten limit już przy równym przeciwniku. Parametry wymagają zestawienia z walkami pełnej treści i późniejszą telemetrią bety.

Progi poziomów są teraz w `content/rules.toml` (`progression.level_xp`), a serwer sprawdza ich liczbę i kolejność. Zmiana progu zmienia wersję treści. Nagrody liczy się z pośrednim `u64`, aby wysoka konfiguracyjna baza PD nie przepełniała mnożenia.

Pozostają 132 karty do około 330, piąta edycja, pełne konfiguracje do balansu i kalibracja po wyborze tempa. Nie oznaczono tych kryteriów jako ukończonych.
