@echo off
rem Sprawdza całe repozytorium: formatowanie, lint, typy, testy i budowanie.
rem Użycie: check.bat           wszystkie kontrole
rem         check.bat -Quick    bez testów end-to-end i testu serwera z bazą
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0scripts\check.ps1" %*
exit /b %ERRORLEVEL%
