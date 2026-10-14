## Co zmienia ten PR

<!-- Krótki opis i krok planu, np. F1.3 (docs/16-plan-wdrozenia.md). -->

## Checklista (docs/14-testy-i-jakosc.md, rozdz. 9)

- [ ] Kod spełnia kryteria akceptacji kroku z `docs/16-plan-wdrozenia.md`.
- [ ] Nowe i zmienione reguły mają testy z identyfikatorami `R-…`; każda nowa karta ma test scenariuszowy.
- [ ] CI jest zielone, budżety wydajności są spełnione, a lokalnie `check.bat` nie zgłasza błędów.
- [ ] Własny przegląd zmian (diff): brak logów debugowych, martwego kodu i zmian spoza zakresu.
- [ ] Dokumentacja jest zaktualizowana (specyfikacja, ADR, protokół), jeśli zmienia się zachowanie.
- [ ] Nowe założenia projektowe są wpisane do rejestru (`docs/02-zasady-gry.md`, rozdz. 15).
- [ ] Migracje bazy działają w trybie expand/contract, jeśli zmienia się schemat.
- [ ] W kodzie i logach nie ma sekretów ani danych osobowych.
