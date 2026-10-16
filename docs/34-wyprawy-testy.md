# Wyprawy: etapy, PD i nagrody

F11.3, 2026-10-16. Portal udostępnia Spacer, Wyprawę i Mękę dla kont z potwierdzonym e-mailem. Każdy etap daje trzech różnych przeciwników do wyboru, a końcowy boss jedną obowiązkową walkę. Trudność ma opis poziomu obok koloru, więc informacja nie zależy wyłącznie od barwy. Interfejs i komunikaty są w PL/EN.

## Plan i rozliczenie

Parametry są w sekcji `expedition` pliku `content/rules.toml` i wpływają na wersję treści. Spacer ma cztery zwykłe etapy i bossa, Wyprawa sześć i bossa, Męka osiem i bossa. Stosowane są poziomy AI i różnice poziomów z rozdziału 5 dokumentu AI, z ograniczeniem poziomu postaci do 1–20. Przeciwnicy wydają swoje PR; talia bossa korzysta z archetypu umagicznień.

Migracja 017 zapisuje prywatny plan, stan wyprawy, wybór przeciwnika i potwierdzenia żądań. Plan zamraża postać, talię, przeciwników, parametry ich AI, pulę nagród oraz prywatne losowanie. API pokazuje wyłącznie bieżących przeciwników i stan uczestnika; nie ujawnia planu, przyszłych etapów ani losowania nagrody. Ponowienie tego samego żądania nie rozpoczyna drugiej walki i nie losuje ponownie planu.

Walka używa wspólnego trwałego aktora i dziennika rund. PD zwykłych etapów pozostają odroczone. Przykład testowy: dwie wygrane Spaceru zbierają 200 PD; wycofanie zachowuje `200 × 2/5 = 80 PD`. Cztery zwykłe wygrane i boss dają w tym scenariuszu 600 PD dopiero w końcowym rozliczeniu. Historia nie zapisuje tych samych PD ponownie przy każdym etapie. Przegrana lub remis kończy wyprawę bez nagrody; wycofać można się między walkami. Uzupełnienia Z-EXP-PD/PROFILE/EMPTY są w rejestrze zasad.

Wyprawa nie zmienia żadnego pola rankingu. Pokonanie bossa wypłaca stałe 5/10/20 Dukatów oraz jeden rozłączny bonus: 40% na nieposiadany szablon właściwej rzadkości, 3% na kartę, 1% na 10/25/50 Dukatów. Pełne posiadanie właściwej kategorii postaci nie powoduje ponownego losowania. Wypłata, kolekcja, odblokowanie i zwolnienie postaci są częścią tej samej transakcji wyniku. Powtórne rozliczenie odczytuje już zapisany wynik.

## Przegląd

Przeciążenie slotów AI nie osłabia przeciwników nagradzanych wypraw do ruchu domyślnego. Aktor czeka na obliczenia, a przy niedostępności ponawia próbę. Historia obserwacji przeciwnika jest odtwarzana z publicznych widoków zakończonych rund. Parametry AI są zapisane w prywatnym miejscu przy stole; usunięcie bieżącej nazwy profilu nie zmienia zapisanego przeciwnika. Starsze dzienniki bez tego pola nadal się odczytują.

Czat wypraw jest dostępny wyłącznie osobom z aktywną wyprawą. Wejście i każda wysyłka sprawdzają uczestnictwo w bazie. Zakończenie lub wycofanie przenosi osobę do Portalu. Test potwierdza, że po opuszczeniu nie otrzymuje ona nowych prywatnych wiadomości.

Cztery testy PostgreSQL obejmują odroczone PD, częściową wypłatę, bossa, rozłączne nagrody, utratę PD, cofnięcie weryfikacji, granicę portfela i serie po 100 równoczesnych wyborów oraz ponowień wycofania. Osobne testy HTTP i WebSocket sprawdzają sesje, CSRF, prywatność i czat. Testy modelu sprawdzają legalne talie, zamrożony plan, brak alternatywy dla bossa oraz wszystkie 10 000 przedziałów losowania: 4000/300/100/5600. Test AI zajmuje wszystkie sloty i potwierdza brak słabszego zastępnika, a następnie odczytuje zapisany profil nieobecny w bieżącym katalogu.

Test przeglądarkowy dwóch kont działa na desktopie i Pixel 5. Obejmuje utraconą odpowiedź rozpoczęcia i ponowienie tego samego identyfikatora, wybór, rzeczywistą walkę, rezygnację z walki, częściowe wycofanie oraz zamknięcie prywatnego czatu. Plansze mobilne zostały obejrzane; nie ma poziomego przepełnienia.

## Ukończenie przez L3

`cargo run --release -p server --example expedition_audit --locked` rozgrywa legalne ruchy silnika bez wymuszania wyników. Bot L3 wybiera przeciwnika na podstawie widocznych statystyk. Wyszukiwanie obejmuje do 64 ziaren na jedną z czterech przygotowanych konfiguracji postaci.

Pierwsza konfiguracja ukończyła wszystkie trudności: szablon 14, droga 0, poziom 20, Ascent, wszystkie dostępne PR i premia reinkarnacji, dar 138, specjalizacja szkolna 3. Ziarna ukończeń: Spacer 0 (5 etapów), Wyprawa 0 (7), Męka 31 (9). Talia: `28, 34, 140, 147, 151, 157, 165, 258, 30, 35, 208, 324, 26, 27, 29, 31, 33, 207, 209, 210`.

To spełnia kryterium ukończenia każdej trudności przez L3; nie jest dowodem łatwości każdej klasy, ani raportem balansu populacji. Wyprawy zwiększają dopływ Dukatów względem modelu ekonomii z raportu 31, który celowo obejmuje tylko arenę i sklep. Ten dopływ wymaga pomiarów F12/F15.

Pełne `scripts/check.ps1` po przeglądzie: wszystkie 19 kroków zielone bez pominięć, 331 s; 86 testów PostgreSQL, 25 Vitest, 24 testy przeglądarkowe, natywne/WASM, SQLx i budżety. Lighthouse: 99/100/100/100. Wersja treści: `2fe5309b47546cda1df29067283d34e881f6c61e58be132e127cc5dfb8225701`. Aplikacja offline zajmuje 199,7/200 KiB, moduł wypraw 4,8/20 KiB. Zapas offline jest mały; dalsze moduły wymagają zachowania podziału kodu i kontroli F13.
