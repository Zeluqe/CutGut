import os
import sys
import json
import hashlib
import urllib.request
import subprocess
from dataclasses import dataclass
from typing import Optional, Callable
from PyQt6.QtCore import QThread, pyqtSignal

import encoder

GITHUB_API_URL = "https://api.github.com/repos/Zeluqe/CutGut/releases"

@dataclass
class ReleaseInfo:
    tag_name: str
    name: str
    body: str
    published_at: str
    asset_url: str
    asset_name: str
    asset_size: int
    prerelease: bool

def check_for_updates(current_version: str, include_prerelease: bool = False) -> Optional[ReleaseInfo]:
    """
    Sprawdza GitHub API pod kątem nowszej wersji aplikacji CutGut.
    Zwraca ReleaseInfo jeśli dostępna jest nowsza wersja, lub None.
    """
    req = urllib.request.Request(
        GITHUB_API_URL,
        headers={
            "User-Agent": "CutGut-App",
            "Accept": "application/vnd.github.v3+json"
        }
    )
    try:
        with urllib.request.urlopen(req, timeout=5) as resp:
            if resp.status != 200:
                return None
            data = json.loads(resp.read().decode('utf-8'))
    except Exception:
        return None

    if not isinstance(data, list) or not data:
        return None

    for rel in data:
        if rel.get('draft', False):
            continue
        is_pre = rel.get('prerelease', False)
        if is_pre and not include_prerelease:
            continue

        tag = rel.get('tag_name', '').lstrip('v')
        if encoder.is_version_newer(current_version, tag):
            # Znajdź asset CutGut.exe
            assets = rel.get('assets', [])
            exe_asset = None
            for a in assets:
                if a.get('name', '').lower() == 'cutgut.exe':
                    exe_asset = a
                    break

            if exe_asset and exe_asset.get('browser_download_url'):
                return ReleaseInfo(
                    tag_name=tag,
                    name=rel.get('name', tag),
                    body=rel.get('body', ''),
                    published_at=rel.get('published_at', ''),
                    asset_url=exe_asset['browser_download_url'],
                    asset_name=exe_asset['name'],
                    asset_size=exe_asset.get('size', 0),
                    prerelease=is_pre
                )
    return None

def download_asset(
    url: str,
    target_path: str,
    progress_callback: Optional[Callable[[int, int], None]] = None,
    cancel_token: Optional[encoder.CancellationToken] = None
) -> str:
    """
    Pobiera plik z URL pod wskazaną ścieżkę z raportowaniem postępu w bajtach.
    """
    req = urllib.request.Request(url, headers={"User-Agent": "CutGut-App"})
    with urllib.request.urlopen(req, timeout=15) as resp:
        total_size = int(resp.headers.get('Content-Length', 0))
        downloaded = 0
        block_size = 64 * 1024
        
        with open(target_path, 'wb') as f:
            while True:
                if cancel_token and cancel_token.cancelled:
                    raise Exception("Pobieranie anulowane przez użytkownika.")
                chunk = resp.read(block_size)
                if not chunk:
                    break
                f.write(chunk)
                downloaded += len(chunk)
                if progress_callback:
                    progress_callback(downloaded, total_size)
                    
    return target_path

def compute_sha256(file_path: str) -> str:
    h = hashlib.sha256()
    with open(file_path, 'rb') as f:
        while chunk := f.read(64 * 1024):
            h.update(chunk)
    return h.hexdigest().lower()

def apply_update_and_restart(new_exe_path: str):
    """
    Tworzy skrypt pomocniczy, który po zamknięciu aplikacji bezpiecznie podmienia CutGut.exe
    na nową wersję i uruchamia program jako czysty proces pulpitu (explorer.exe).
    """
    current_exe = sys.executable if getattr(sys, 'frozen', False) else os.path.join(encoder.get_base_dir(), 'CutGut.exe')
    current_pid = os.getpid()
    
    base_dir = os.path.dirname(os.path.abspath(current_exe))
    bak_path = os.path.join(base_dir, 'CutGut.exe.bak')

    ps_command = f"""
    $pidToWait = {current_pid}
    $curExe = '{current_exe}'
    $bakExe = '{bak_path}'
    $newExe = '{new_exe_path}'

    try {{
        Wait-Process -Id $pidToWait -Timeout 15 -ErrorAction SilentlyContinue
    }} catch {{}}
    Start-Sleep -Seconds 1

    if (Test-Path $bakExe) {{ Remove-Item -Force $bakExe -ErrorAction SilentlyContinue }}
    if (Test-Path $curExe) {{ Move-Item -Force $curExe $bakExe -ErrorAction SilentlyContinue }}
    if (Test-Path $newExe) {{ Move-Item -Force $newExe $curExe -ErrorAction SilentlyContinue }}

    Start-Sleep -Milliseconds 500
    Start-Process -FilePath $curExe
    """

    DETACHED_PROCESS = 0x00000008
    CREATE_NEW_PROCESS_GROUP = 0x00000200

    try:
        subprocess.Popen(
            [
                'powershell.exe',
                '-NoProfile',
                '-NonInteractive',
                '-WindowStyle', 'Hidden',
                '-Command', ps_command
            ],
            creationflags=DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP | encoder.CREATE_NO_WINDOW,
            close_fds=True
        )
    except Exception:
        # Fallback do .bat z odpaleniem przez explorer.exe
        bat_path = os.path.join(base_dir, 'cutgut_updater.bat')
        bat_content = f"""@echo off
timeout /t 2 /nobreak >nul
if exist "{bak_path}" del /f /q "{bak_path}"
if exist "{current_exe}" move /y "{current_exe}" "{bak_path}"
move /y "{new_exe_path}" "{current_exe}"
explorer.exe "{current_exe}"
del "%~f0" & exit
"""
        with open(bat_path, 'w', encoding='utf-8') as f:
            f.write(bat_content)
        subprocess.Popen(
            ['cmd.exe', '/c', bat_path],
            creationflags=DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP | encoder.CREATE_NO_WINDOW,
            close_fds=True
        )

    sys.exit(0)


class CheckUpdateWorker(QThread):
    finished_signal = pyqtSignal(object) # Optional[ReleaseInfo]

    def __init__(self, current_version: str, include_prerelease: bool = False):
        super().__init__()
        self.current_version = current_version
        self.include_prerelease = include_prerelease

    def run(self):
        rel = check_for_updates(self.current_version, self.include_prerelease)
        self.finished_signal.emit(rel)

class DownloadUpdateWorker(QThread):
    progress_signal = pyqtSignal(int, int) # downloaded, total
    finished_signal = pyqtSignal(str)     # downloaded file path
    error_signal = pyqtSignal(str)

    def __init__(self, asset_url: str, target_path: str):
        super().__init__()
        self.asset_url = asset_url
        self.target_path = target_path
        self.cancel_token = encoder.CancellationToken()

    def run(self):
        try:
            p = download_asset(
                self.asset_url,
                self.target_path,
                progress_callback=lambda d, t: self.progress_signal.emit(d, t),
                cancel_token=self.cancel_token
            )
            self.finished_signal.emit(p)
        except Exception as e:
            self.error_signal.emit(str(e))

    def cancel(self):
        self.cancel_token.cancel()
