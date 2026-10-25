@echo off
setlocal
cd /d "%~dp0"
if not exist venv\Scripts\activate.bat ( echo [formvfx] engine venv missing, run formvfx.bat install & exit /b 1 )
call venv\Scripts\activate.bat
if not defined FORMVFX_PORT set "FORMVFX_PORT=8250"
uvicorn app.main:app --reload --reload-dir app --host 127.0.0.1 --port %FORMVFX_PORT%
