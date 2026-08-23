@echo off
cd /d "%~dp0"
:: Uruchomienie profesjonalnej wersji PyQt6
start "" .\venv\Scripts\pythonw.exe gui.py
