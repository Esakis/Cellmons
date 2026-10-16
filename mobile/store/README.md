# Materiały do testu zamkniętego Google Play

To przygotowanie wydania, nie opublikowana karta sklepu. Teksty PL/EN, ikona 512×512 i grafika 1024×500 są gotowe do przeglądu. `e2e/render-android-art.mjs` odtwarza grafiki z własnego `web/public/icon.svg`. Zrzuty pochodzą z emulatora i kont testowych; przed publikacją zastąpić je zaakceptowanymi ujęciami z urządzeń.

Właściciel musi podać trwały `ANDROID_APPLICATION_ID`, konto organizacji, dane wydawcy, kontakt wsparcia i publiczne adresy polityki prywatności oraz usuwania konta. `dev.game.training` służy wyłącznie do debugowania. Nie nadano właścicielowi przypadkowego identyfikatora aplikacji.

## Bezpieczeństwo danych — inwentaryzacja do formularza

| Dane rzeczywiście używane przez grę | Cel | Uwagi do deklaracji |
|---|---|---|
| Login, e-mail, UUID konta | Konto i obsługa gry | Dane osobowe / adres e-mail / identyfikatory użytkownika |
| Kolekcja, postacie, wyniki, Dukaty | Funkcje gry | Aktywność w aplikacji; nie ma danych płatniczych |
| Wiadomości czatu i poczty, zgłoszenia | Komunikacja, moderacja | Wiadomości użytkowników; dostęp ograniczony serwerem |
| Skrót IP z sekretną solą, user agent, sesje | Bezpieczeństwo i nadużycia | Zweryfikować przyporządkowanie identyfikatorów w aktualnym formularzu; brak geolokalizacji i ID reklamowego |
| Dane gościa | Lokalny trening | Brak przesyłania do API i zapisywania postępu |

Nie deklarować zbierania diagnostyki przez zewnętrzny SDK, którego w aplikacji nie ma. Nie ma reklam, analityki reklamowej, zakupów ani udostępniania reklamodawcom. Hosting i dostawca poczty muszą zostać ujawnieni zgodnie z rzeczywistą konfiguracją i umowami. Szyfrowanie transmisji: HTTPS/WSS; magazyn tokenu: Android Keystore, bez kopii Androida. Eksport i usuwanie konta zaimplementowano w F13 i przetestowano w przeglądarce oraz przez SAF Androida. Rzeczywisty publiczny adres oraz odbiór prawny nadal są bramką publikacji; tej tabeli nie należy traktować jako wypełnionej deklaracji Play.

## IARC i test zamknięty

Nie przypisano samodzielnie kategorii wiekowej. Kwestionariusz musi uwzględnić fantastyczną walkę, czat i treści użytkowników, losowe zestawy za walutę zdobywaną w grze oraz wpisowe i nagrody turniejowe. Brak sprzedaży waluty za pieniądze nie zastępuje odpowiedzi na pytania formularza.

Przed promocją do produkcji: zespół testerów, konto recenzenta, raport przedpremierowy Play, macierz fizycznych urządzeń i zgoda na materiały graficzne. Nie utworzono konta Play ani nie wysłano wersji do sklepu.

Weryfikacja zasad 2026-10-16: [target API — wymagane co najmniej 36](https://support.google.com/googleplay/android-developer/answer/11926878?hl=en-EN), [usuwanie konta w aplikacji i przez WWW](https://support.google.com/googleplay/android-developer/answer/13327111?hl=en), [dane użytkownika](https://support.google.com/googleplay/android-developer/answer/10144311?hl=en-gb). Sprawdzić ponownie przed wysłaniem wydania.
