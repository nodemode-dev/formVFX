@echo off
setlocal
cd /d "%~dp0.."
start "formvfx engine" cmd /k "cd /d %cd%\backend && boot_backend.bat"
start "formvfx interface" cmd /k "cd /d %cd%\frontend && boot_frontend.bat"
echo [formvfx] interface http://localhost:5230 and engine http://127.0.0.1:8230
