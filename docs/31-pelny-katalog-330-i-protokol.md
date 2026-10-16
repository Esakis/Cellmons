# Pełny katalog i protokół klienta

Stan implementacji F10.3, 2026-10-15: 330 kart, 31 szablonów, 62 drogi i sześć zestawów startowych. To komplet definicji i ilustracji do przeglądu; nie deklaracja zatwierdzenia grafik ani wyrównanego balansu wszystkich klas.

| Edycja | Karty | Brązowe | Srebrne | Złote |
|---|---:|---:|---:|---:|
| Core | 72 | 72 | 0 | 0 |
| Forge | 60 | 0 | 30 | 30 |
| Relic | 66 | 9 | 54 | 3 |
| Covenant | 66 | 0 | 42 | 24 |
| Eclipse | 66 | 30 | 24 | 12 |

Po pierwszej paczce Covenant dodano cztery paczki po 33 karty: Eclipse 199–231, Relic 232–264, Covenant 265–297 i Eclipse 298–330. Każda ma teksty PL/EN, jawne wymagania i koszty, scenariusze liczbowe oraz galerię SVG. Dane źródłowe pozostają w TOML; klient i silnik korzystają z tego samego kompilatora oraz wersji treści.

## Przegląd paczek

Pierwszy Eclipse dodaje narzędzia dostępne od pierwszego poziomu szkoły, sześć przeciwzaklęć z kosztowniejszym efektem dodatkowym i trzy czary działające od trzeciej rundy. Scenariusze potwierdzają anulowanie rzeczywistego czaru przeciwnika, koszty oraz brak efektu przy zbyt wczesnym rzuceniu.

Drugi Relic rozszerza sprzężenia, przewagę procentowego Ż, dobieranie i odrzucanie, usuwanie umagicznień i efekty wyzwalane trafieniem, blokiem lub obrażeniami. Sprzężenia sprawdzono przy poziomach szkoły 1/2/3/4, włącznie z dodatkowym kosztem K/W. Warunek przewagi nie działa przy równym procencie Ż. Rozproszenie własnej postaci usuwa także korzystny dar: scenariusz sprawdza zniknięcie jego stałej obrony.

Drugi Covenant daje piętnaście par szkół, sześć kosztownych podniesień szkoły 3→4 i dwanaście wymian statystyk lub zasobów. Scenariusze sprawdzają sloty wymuszonego trafienia i ataku nieblokowalnego, stronę wysysania, ujemne skutki oraz zatrzymanie szkoły na poziomie 4. Nowe czary nie zmieniają zasady, że wyłącznie droga Perfekcji cienia osiąga szkołę 4 bez czaru.

Drugi Eclipse zawiera czary wymagające wyczerpania W przeciwnika, operacje na ręce, osłony 75%, jednorazowe przeciwzaklęcia i trzy karty od rundy czwartej. Sprawdzono odrębnie 0 i 1 W przeciwnika, zaokrąglenie obrażeń każdego trafienia, limit pięciu kart w ręce i odmowę drugiego użycia interwencji.

Wartości są własnym projektem na podstawie przetestowanych rodzin efektów. Nie przedstawia się ich jako odzyskanych opisów oryginalnych kart. Wszystkie nowe karty są w publicznej puli zestawów właściwej rzadkości; zakup nadal zamraża pulę i wersję przed późniejszym otwarciem.

## Poprawki przeglądu

- Kompilator walidował zmienną Fluent `$level`, lecz nie eksportował progu sprzężenia w parametrach klienta. Dodano eksport i regresję kompilatora; test PL/EN formatuje opis każdej z 330 kart bez brakujących zmiennych.
- Instrukcja wysypiska i podgląd punktów używają bieżących reguł `junkyard`, zamiast powtarzać wartości w kodzie i tłumaczeniach.
- Zarejestrowana kolekcja czeka na katalog dróg przed publikacją stanu. Zmiana konta podczas oczekiwania unieważnia operację przed wywołaniem API. Gość zachowuje kolekcję w RAM i nie pobiera danych konta.
- Nazwy i profile dróg oraz ekran reinkarnacji są ładowane dla kont. Ikony SVG są współdzielone w kodzie, a trening i kolekcja korzystają z jednego modułu offline. Ilustracje kart zachowują osobny moduł na żądanie.

## Protobuf i rozmiar

Klient nie potrzebuje w przeglądarce pełnej refleksji nad deskryptorami. Ma wygenerowane metadane pól, ograniczony kodek proto3 i standardowe `TextEncoder`/`TextDecoder`. Kod serwera nadal generuje oficjalny zestaw narzędzi Protobuf. Nie zmieniono numerów pól ani protokołu.

Testy porównują bajty i wartości z oficjalnym `@bufbuild/protobuf`: wszystkie 72 wiadomości online, 1000 pełnych par kopert, 600 próbek treningu oraz 2000 zestawów prymitywów. Obejmują liczby ujemne i zigzag, granice 64 bitów, Unicode, scalanie fragmentów wiadomości, listy pakowane i zwykłe, nieznane pola, niepełne ramki, niepoprawny UTF-8 i przepełnienia varint. Grupy proto2 są odrzucane; nie występują w schemacie proto3. Limit wejścia to 128 KiB dla workera i 256 KiB online, głębokość zagnieżdżenia 16.

Zmierzony pakiet pełnej treści: około 199/200 KiB offline, 69,2/90 KiB WASM, 40,4/80 KiB ilustracji i 35,2/45 KiB modułów konta oraz lobby. Limity nie zostały zwiększone. Wartości mogą nieznacznie zmieniać się z hashem wydania. Suma JS trasy mapy wymaga osobnego pomiaru w F13: sam test startowego modułu nie dowodzi spełnienia limitu każdej trasy.

## Balans i odbiór

Przegląd `audit-card-overlap.py` porównał koszty, wymagania i proste wektory efektów przy jednakowych warunkach, wyzwalaczach i słowach kluczowych. Dziewięć wskazań obejmowało osiem kart. Poprawiono `fury_catalyst`, `light_balance`, `marrow_oath`, `dew_censer`, `impact_rivet`, `verdant_litany`, `radiant_clause` i `twilight_balance`, wraz z opisami i scenariuszami. Ponowny skan 330 kart daje zero wskazań w tym ograniczonym modelu. Nie obejmuje wszystkich kontekstów walki i nie dowodzi globalnego braku dominacji.

`roster_optimized` dodaje audyt L2 z wydanymi PR i powtarzalnym przeszukiwaniem legalnych talii. Próby wyboru talii i PR korzystają z innych ziaren niż końcowe pojedynki każdej pary klas. Wyszukiwanie ma skończony budżet i nie jest dowodem optymalności. Nie obejmuje jeszcze wszystkich kombinacji dróg, specjalizacji i darów; wyniki trzeba odróżniać od wcześniejszego L1 bez PR.

Kalibracja tempa opisana w [raporcie 30](30-covenant-i-tempo-rozwoju.md) zachowuje bazę PD 100 do określenia docelowego tempa. Publiczna beta i pomiary rzeczywistych karier nadal pozostają przed wdrożeniem.

Po poprawkach rozegrano 180 000 pojedynków kontrolnych: 25 zwykłych i wysypiskowych klas, poziomy 1/10/20, po 200 walk na parę. Wybór talii i wydanie wszystkich PR poprzedzało 88 200 walk z osobnymi ziarnami. Na poziomie 20 wynik punktowy (wygrana + połowa remisów) wynosił m.in. 63,7% dla `tideform`, 63,5% dla `bone_sentinel`, 63,1% dla `dune_wanderer`, 38,2% dla `aether_seer`, 35,0% dla `cinder_mendicant`, 28,2% dla `glass_channeler` i 24,9% dla `star_adept`. To nierozwiązany sygnał do kalibracji F10.5 i bety. Przeszukiwanie ma tylko 48 prób na klasę i poziom; nie uzasadnia automatycznej zmiany historycznych krzywych ani uznania znalezionych talii za optymalne.

`audit-economy.py` modeluje 100 karier, 20 walk dziennie, 50% wygranych, obecne limity nagród, zestawy i wysypisko. Strategie mają identyczne harmonogramy wygranych i osobne losowanie łupów. Przy zakupach dobieranych zachłannie do brakujących kopii mediana zebrania połowy kart wynosi 44 dni, wszystkich pojedynczych kart 595 dni, czterech kopii każdej 1150 dni (p10–p90: 997–1297), trzech klas wysypiska 221 dni. Kupowanie wyłącznie zestawu podstawowego daje medianę 1178 dni do czterech kopii każdej karty. Model odrzuca nadmiar ponad cztery kopie, uwzględnia zwroty wysypiska i kontroluje zachowanie waluty. Pomija handel oraz nagrody wypraw i turniejów; nie jest prognozą zachowania graczy ani dowodem optymalności strategii. Wyniki wskazują potrzebę ustalenia docelowego tempa, zamiast oznaczenia F10.5 jako zakończonego.

Pełna kontrola po tych poprawkach: `scripts/check.ps1`, wszystkie 19 kroków bez pominięć, około 214 s. Obejmuje 64 testy PostgreSQL, 25 testów Vitest, 24 testy przeglądarkowe, zgodność natywne/WASM i Lighthouse 100/100/100/100. Wersja treści tego pomiaru: `3720e4e803f0fad2c2961a3e9db97d8dda1dc1eb9f63739364a9afb23b4f890d`; późniejsze reguły klanów zmieniają hash, bez zmiany danych walki.

Galerie: `art/eclipse-review.html`, `art/relic-second-review.html`, `art/covenant-second-review.html`, `art/eclipse-second-review.html`. Wszystkie 132 nowe ilustracje zostały zdekodowane w Chromium i obejrzane na planszach. Poprawiono kontrast emblematów. Drugie paczki celowo współdzielą własną geometrię przedmiotów i szkół projektu; nie używają materiałów zewnętrznych. Wpisy pochodzenia są w `art/PROVENANCE.md`. Odbiór właściciela pozostaje otwarty.
