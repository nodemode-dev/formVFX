@echo off
setlocal
cd /d "%~dp0.."
if not exist frontend\dist\index.html call setup\build_frontend.bat
call backend\venv\Scripts\activate.bat
set "FORMVFX_DIST=%cd%\frontend\dist"
cd backend
if not defined FORMVFX_PORT set "FORMVFX_PORT=8250"
echo [formvfx] serving http://127.0.0.1:%FORMVFX_PORT%
uvicorn app.main:app --host 127.0.0.1 --port %FORMVFX_PORT%
