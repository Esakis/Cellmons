# Dane referencyjne z archiwum

Pliki w tym katalogu to dane zebrane z archiwalnych stron oryginalnej gry Cellmons (Internet Archive) i z fanowskiego serwisu Cellmons Guru. Służą jako **materiał badawczy**: punkt odniesienia przy projektowaniu mechaniki, balansu i zawartości naszej gry.

> **Uwaga prawna.** Nazwy kart, postaci, dróg i darów są elementami oryginalnej gry (prawa: Pogram / Toolzz & Toyzzz lub ich następcy prawni). Zgodnie z decyzją projektową (ADR-012, `../05-decyzje-architektoniczne.md`) **nie przenosimy ich do produktu**. Treść gry dostaje własne nazwy, opisy i grafiki. Liczby (statystyki, koszty, progi) traktujemy jako punkt wyjścia do balansu, nie jako kopię 1:1.

## Pliki

| Plik | Zawartość | Wiersze | Źródło | Wiarygodność |
|---|---|---|---|---|
| `karty_klasyczne.csv` | Pełna lista kart wersji klasycznej: `id`, `nazwa`, `rzadkosc`, `wymagania` (szkoła:poziom), koszty z wersji 2010 (jeśli znane) | 338 | lista kart tenbit.pl (2005–2006) + lista czarów cellmons.com (2010) | lista, rzadkość, wymagania: wysoka. Koszty: średnia (pochodzą z 2010, 186 z 338 kart) |
| `postacie.csv` | 31 szablonów postaci: kategoria rzadkości z wersji klasycznej, postać startowa szkoły, szkoły na poz. 1, statystyki bazowe | 31 | strona „Postacie” cellmons.com (2010) + FAQ Cellmons Guru (kategorie) | wysoka (statystyki Lamberta, Masakratora, Mnicha i innych zgodne z danymi fanów z 2007) |
| `rozwoj_postaci.csv` | Tabele rozwoju 16 postaci/dróg (pełne, poz. 1–20) i 1 częściowa (Elektrykus, poz. 1–2): przyrosty statystyk na każdym poziomie, punkty rozwoju, poziomy szkół. Wiersz `Suma` to stan na poz. 20 bez wydanych punktów rozwoju | 338 | Cellmons Guru, „rozwój postaci” (2006–2007) | wysoka (dane spisywane przez graczy) |
| `drogi_rozwoju.csv` | Drogi (1. reinkarnacja), specjalizacje (2.) i dary (3.) dla każdej postaci, w tym postaci z wysypiska i cieni | 62 | Cellmons Guru, „reinkarnacje” (2007) | wysoka |
| `progi_doswiadczenia.csv` | Punkty doświadczenia (PD) wymagane na poziomy 1–20 i do reinkarnacji | 21 | oficjalna instrukcja + FAQ Cellmons Guru | wysoka (zgodne w 3 źródłach) |
| `zestawy_startowe.csv` | Karty, które dostaje nowe konto zależnie od wybranej szkoły (36–40 kart na szkołę) | 73 | Cellmons Guru, „zestawy początkowe” (2008) | wysoka |

## Kodowanie i konwencje

- UTF-8, separator `,`, nagłówek w pierwszym wierszu, końce linii `\n`.
- Szkoły zapisujemy kluczami: `walka`, `kaplanstwo`, `elementalizm`, `magia`, `nekromancja`, `technologia`.
- `wymagania` / `szkoly_poziom_1`: lista `szkoła:poziom` rozdzielona spacjami. Poziom 1–4 (Nowicjusz, Adept, Mistrz, Arcymistrz).
- `rzadkosc`: `brazowa`, `srebrna`, `zlota`.
- `kategoria_klasyczna` (postacie): `pospolita`, `niepospolita`, `rzadka` (tak oryginał klasyfikował postacie z wypraw), `wysypisko` (do zdobycia tylko w Wieży Maga), `cien` (powstają przez poświęcenie postaci Ascent).
- W `rozwoj_postaci.csv` wiersz `1` zawiera wartości bazowe, a kolejne wiersze przyrosty (`+1`, `0`). Kolumna `szkoly` jest wypełniona tylko na poziomach, na których zmienia się zestaw szkół.

## Jak powstały te dane

1. Indeksy Internet Archive (CDX) dla domen `cellmons.com`, `cellmons.tenbit.pl` i `cellmons.waw.pl`.
2. Pobranie zarchiwizowanych stron (tryb `id_`, surowy HTML) i konwersja kodowania (ISO-8859-2 / UTF-8).
3. Parsowanie tabel. Wymagania kart odczytane z ikon szkół (`/i/cellmons/wymagania/{szkoła}/{rzadkość}.gif`), mapowanie numerów szkół potwierdzone na 186 kartach zgodnych z listą z 2010.
4. Kontrola spójności: wymagania z 2005 i 2010 zgodne w 186 z 186 kart, statystyki postaci zgodne między źródłami.

## Znane luki

- **Teksty efektów kart:** w archiwum są tylko dla ok. 20 kart (strony `effect_detail` z 2010). Opisy pozostałych kart były na obrazkach, których archiwum nie zachowało. Efekty trzeba zaprojektować na nowo (patrz `../03-tresc-gry.md`, rozdz. 5.3 i 5.4).
- **Koszty kart:** znane dla 186 kart, tylko z wersji 2010. Mogły się różnić od klasycznych.
- **Wzór na PD za walkę:** nieznany (kalkulator „Expiarz” nie zachował się). Patrz założenie Z-XP w `../02-zasady-gry.md`.
- **Tabele rozwoju:** pełne dla 16 postaci/dróg (7 postaci bazowych i 9 dróg po reinkarnacji), częściowa dla Elektrykusa. Dla pozostałych trzeba je zaprojektować według tych wzorców.
