# F8: postacie, rozwój i kolekcja

Implementacja z 2026-10-15 obejmuje F8.1–F8.5. Konto używa `/api/game`; gość ma osobny stan w RAM, bez wywołań API i bez zapisu do magazynów przeglądarki.

## Reguły i transakcje

Gracz ma do 10 żywych postaci z posiadanych szablonów. Nowa postać ma poziom 1, zero PD i pustą talię. Aktywacja jest atomowa; aktywnej postaci nie można usunąć. Usunięcie pozostawia rekord historii i zwalnia slot, zachowując szablon i kolekcję kart.

Wydanie PR sprawdza sumę i aktualną rewizję postaci. Jeden PR daje +1 do ataku/obrony albo +7 do życia/wytrzymałości. Powtórzenie starego żądania nie wydaje punktów drugi raz. Awans i przyznanie nowych PR w transakcji wyniku również zmieniają rewizję.

Talia ma do 20 kart i do czterech kopii, ograniczonych dodatkowo własną kolekcją. Ta sama karta może być używana w taliach wielu postaci. Szkoły nie ograniczają dodawania do talii — weryfikacja wymagań następuje przy rzucaniu (R-TAL-04). Pusta lub niepełna talia korzysta z losowego uzupełnienia silnika. Zestawy mają unikalne nazwy i limit 10. Zmiany talii i zestawów sprawdzają rewizje, aby drugie okno nie nadpisywało nowego stanu.

Hub szereguje mutacje z wyzwaniami, a transakcja pod blokadą użytkownika ponownie sprawdza trwałą blokadę walki. Wysłane wyzwanie i aktywny pojedynek uniemożliwiają mutację. Prawidłowa zmiana anuluje przychodzące wyzwania i odświeża profil w lobby. Endpoint ma limity 60 odczytów i 60 mutacji na minutę dla użytkownika. Korzysta z tej samej autoryzacji, kontroli Origin, nagłówka żądania, CORS i `no-store` co konta.

`game::prune` jest częścią transakcji przyszłej operacji ekonomii zmniejszającej kolekcję. Przy zachowaniu kolejności usuwa nadmiarowe kopie ze wszystkich talii i zestawów, bez przyznawania kart. Nie jest publiczną operacją zmieniającą stan kolekcji.

## Nagrody arenowe

Nagroda szablonu jest częścią tej samej transakcji co wynik, PD i ranking. Osobny licznik wygranych arenowych nie obejmuje treningów ani przyszłych innych trybów. Gwarancje dotyczą zwycięstw 5, 15 i 30. Poza nimi szansa wynosi 5000 na milion. Losowanie wynika z ujawnianego po walce ziarna i identyfikatora gracza; ponowienie transakcji nie zmienia nagrody.

Wybór pomija posiadane szablony, cienie i szablony wysypiska. Wagi kategorii to 70/25/5. Jeżeli aktualna częściowa treść nie zawiera brakującej postaci w wylosowanej kategorii, wybieramy spośród wszystkich brakujących zwykłych szablonów. Gdy gracz ma wszystkie, nagroda jest pusta. W próbie 50 000 ziaren wypadło 238 nagród, czyli 0,476%; test akceptuje zakres 0,4–0,6%. Gwarancje i brak duplikatów sprawdza odrębna część testu.

Migracja 008 dodaje rewizje, licznik areny oraz historyczną nagrodę do meczu. Migracja 009 zapisuje gotową publiczną powtórkę i końcowy widok każdego uczestnika w transakcji wyniku. Zakończona historia działa po aktualizacji silnika/treści bez uruchamiania poprzedniego silnika. Aktywne walki nadal wymagają tej samej wersji wykonawcy lub wygaszenia przy wdrożeniu. Starsze mecze sprzed migracji 009 mają odczyt awaryjny z dziennika, o ile jego wersja jest dostępna.

## Klient i gość

Trasa `#/collection` zawiera pulę postaci, rozwój, konstruktor talii, zestawy i bibliotekę z wyszukiwaniem, rzadkością, filtrem posiadania oraz wymagań szkół. Usuwanie postaci i wydanie PR wymagają potwierdzenia konkretnej zmiany. Niezapisana talia jest sygnalizowana przy wyjściu. Niezgodna wersja treści daje komunikat o potrzebie odświeżenia aplikacji.

Gość otrzymuje demonstracyjne szablony i karty w pamięci. Może utworzyć postacie i własną talię oraz przetestować ją offline. `TrainingCustomPlayer` przenosi do WASM wyłącznie lokalny poziom, wydane PR i talię; ten komunikat nie jest używany przez serwer online. Dane treningu są oddzielone między sesjami konta/gościa. Odświeżenie usuwa stan gościa. Wydawanie PR gościa wymaga posiadania PR; sam trening offline nie przyznaje nagród online.

## Weryfikacja i przegląd

Testy serwera sprawdzają autoryzację, sloty i równoczesne utworzenie ostatniej postaci, aktywację/usuwanie, zachowanie kolekcji, PR, powtórzenia starych żądań, własność i limity talii, nazwy zestawów, atomowe przycinanie, talię z obcej szkoły, blokadę wyzwania/walki i gwarantowane odblokowanie. Test historyczny zmienia oznaczenie wersji prywatnego dziennika i nadal odczytuje końcowy widok oraz powtórkę.

Playwright na desktopie i Pixel 5 obejmuje „nowa postać → aktywacja → talia → zestaw → walka online” na prawdziwym koncie oraz lokalną kolekcję gościa, trening bez sieci i wyczyszczenie po odświeżeniu. Test gościa sprawdza zero żądań API/WebSocket i brak danych kolekcji w magazynach. Test WASM sprawdza własne 20 kart, poziom oraz premie PR. Osobny test web sprawdza izolację kolejnych sesji gościa i ponowne użycie slotu bez dwóch aktywnych postaci.

Przegląd poprawił zawijanie nowej nawigacji na małym ekranie: wcześniejsze przepełnienie powiększało viewport i zakłócało trafianie w przyciski. Kontrola szerokości porównuje teraz dokument z `clientWidth`. Poprawiono także zerowanie formularza po wydaniu PR, ponowne użycie slotu gościa oraz ponawiane sprzątanie rezerwacji po chwilowej awarii bazy.

Zrzuty znajdują się w `target/collection-*.png`; pełny przebieg kontroli w `target/f8-complete-check.log`. Docelowe środowisko, fizyczne telefony i odbiór przez publiczny Internet pozostają odrębnymi testami, opisanymi w planie.

W zbiorczym przebiegu przeszły testy Rust/WASM, 46 testów PostgreSQL, 15 testów Vitest i 24 E2E. Pierwszy pomiar Lighthouse wykrył przesunięcie układu nagłówka po załadowaniu fontu. Stała siatka nawigacji na telefonie usunęła zmianę liczby wierszy; ponowiony pomiar: **100/100/100/100** (`target/f8-lighthouse-final.log`). Ostatni budżet: **195,7/200 KiB offline**, konto/lobby 20,1/45 KiB.
