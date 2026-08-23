@echo off
cd /d "%~dp0"
title CutGut Launcher

if exist ".\venv\Scripts\pythonw.exe" (
    start "" ".\venv\Scripts\pythonw.exe" gui.py
    exit /b 0
)

if exist ".\venv\Scripts\python.exe" (
    start "" ".\venv\Scripts\python.exe" gui.py
    exit /b 0
)

where python >nul 2>nul
if %ERRORLEVEL% equ 0 (
    start "" python gui.py
    exit /b 0
)

echo [BLAD] Nie znaleziono srodowiska Python ani venv!
echo Zainstaluj Python 3.10+ lub utworz srodowisko: python -m venv venv
pause

