# F15 — utrzymanie i dalsze wydania

Gotowe są narzędzia utrzymania i procedury. Eksploatacja jest pracą ciągłą po rzeczywistym uruchomieniu usługi; nie można odhaczyć przyszłych miesięcy na podstawie lokalnych testów.

## Harmonogram

| Częstotliwość | Automatyzacja | Odbiór operatora |
|---|---|---|
| Co 10 minut | `server maintenance`: retencja, usuwanie po karencji, sygnały nadużyć, uzgodnienie księgi | brak zaległości i rozbieżności; ręczne rozstrzygnięcie sygnałów |
| Codziennie | pełna kopia + sprawdzenie WAL, retencja plików, aktualizacja zakresów Cloudflare | aktualna kopia, wolne miejsce, działające alarmy |
| Co tydzień | `server telemetry-report`, audyt zależności | przegląd balansu, PD, źródeł/ujść Dukatów i zgłoszeń |
| Co miesiąc | odtworzenie PITR na osobnej maszynie | wynik granicy czasu, integralność, rzeczywiste RTO <1 h i RPO ≤5 min |
| Przed każdym wydaniem | CI, skany, rozmiary, e2e, kopia i bariera aktywności | zgodność silnika/treści, materiały dla graczy, decyzja o publikacji |
| Corocznie i przed wysłaniem AAB | przegląd wymagań Android/Play | aktualny target API, Data safety, IARC, podpisy i test fizycznych urządzeń |

Jednostki w `deploy/systemd/` i instrukcja w `deploy/README.md` wskazują konkretne polecenia. Timer odtwarzania instaluje się na oddzielnym hoście, nie na produkcji.

## Telemetria i balans

Raport tygodniowy jest tylko do odczytu, ma spójny snapshot i limit czasu. Agreguje konta aktywne przez sesje, walki, PD i przepływy Dukatów; wyniki postaci i użytych kart rozdziela według wersji silnika, treści i trybu. Grupy poniżej 20 gier są pomijane. Analiza bierze maksymalnie 20 000 najnowszych powtórek, jawnie podaje obcięcie i liczbę uszkodzonych rekordów. Bigint waluty jest tekstem, aby uniknąć utraty precyzji.

Próba rzucenia karty jest liczona raz dla strony/karty/walki, również przy skontrowaniu lub nieudanym rzuceniu. Procent wygranych graczy używających karty nie dowodzi jej przyczynowej siły. Operator porównuje także pary postaci, poziomy, rodzaj talii, dobór przeciwników i wielkość próby. Gość nie jest śledzony.

Proces poprawki balansu: zapisz problem i hipotezę → porównaj telemetrię i zgłoszenia → uruchom macierze/symulację progresji → przygotuj małą zmianę wersjonowanej treści i testy scenariuszowe → code review → staging i próba graczy → opublikuj z opisem oraz planem wycofania. Nie zmieniaj automatycznie sald, kar ani parametrów na podstawie pojedynczego alertu. Obecne bazowe 100 PD pozostaje bez samowolnej korekty; odbiór tempa F10.5 wymaga decyzji projektowej i gry ludzi.

Nowa edycja: ustal zakres i identyfikatory → TOML i PL/EN → test każdej karty → grafiki i wpis pochodzenia → kompilator treści, budżety klienta, macierz balansu → przegląd nazw/grafik → beta → zgodne wydanie serwera, przeglądarki i paczki Android. Zachowaj historyczne wersje potrzebne do powtórek. Nie uruchamiaj jednocześnie niezgodnych aktorów nad aktywnymi dziennikami.

## Obsługa incydentu

Rozbieżność księgi oznacza zatrzymanie nowych operacji ekonomicznych i analizę, bez ręcznego edytowania salda. Naprawa wymaga zbilansowanej transakcji i audytu. Błędy kopii lub archiwizacji blokują wydanie. Przy odtwarzaniu bazy uwzględnij wnioski prywatności złożone po punkcie przywrócenia; nie przywracaj dostępu dawnymi sesjami. Szczegóły R1–R13 są w instrukcji wdrożenia i raporcie Androida.

Przy wadliwej paczce web Android odrzuca niezgodny podpis/hash/wersję i ma lokalny rollback próby. Nie obniżaj zdalnie numeru manifestu: działa trwały próg wersji. Naprawione wydanie musi mieć nowy, wyższy numer. Przed publikacją sprawdź zgodność paczki i uruchomionego serwera.

Płatności, zewnętrzne logowanie, iOS i push pozostają opcjonalnymi decyzjami właściciela. Nie dodano fikcyjnych płatności ani kont dostawców.

## Granica ukończenia prac lokalnych

Implementacja obejmuje plan do F15, wraz z przeglądem i poprawkami. Otwarte odbiory publicznego startu są zestawione w [raporcie F13](37-utwardzenie-i-prywatnosc.md) i [raporcie F14](38-wdrozenie-i-odtwarzanie.md). Nie są one zgodą prawną ani potwierdzeniem przeprowadzenia testów ludzi. Do czasu ich wykonania aplikacja pozostaje przygotowanym lokalnie kandydatem do bety.
