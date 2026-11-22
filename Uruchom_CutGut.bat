@echo off
cd /d "%~dp0"
title CutGut

rem Start the freshest build: cargo puts it in cutgut-gui\target\release.
rem CutGut.exe next to this file is only the fallback, so an old copy there
rem cannot silently shadow a new build again.
set "APP=cutgut-gui\target\release\cutgut-gui.exe"
if not exist "%APP%" set "APP=CutGut.exe"

if not exist "%APP%" (
    echo [BLAD] Nie znaleziono CutGut.exe
    echo Zbuduj aplikacje: cargo build --release --manifest-path cutgut-gui\Cargo.toml
    echo i skopiuj cutgut-gui\target\release\cutgut-gui.exe jako CutGut.exe
    pause
    exit /b 1
)

echo Uruchamiam %APP%
start "" "%~dp0%APP%"
