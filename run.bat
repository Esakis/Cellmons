@echo off
rem Uruchamia grę lokalnie: baza danych (Docker), serwer gry, klient i przeglądarka.
rem Użycie: run.bat               uruchomienie
rem         run.bat -NoBrowser    bez otwierania przeglądarki
rem         run.bat -Stop         zatrzymanie bazy danych
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0scripts\run.ps1" %*
exit /b %ERRORLEVEL%
