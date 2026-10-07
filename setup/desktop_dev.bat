@echo off
setlocal
cd /d "%~dp0.."
call setup\freeze_backend.bat || exit /b 1
call frontend\node_modules\.bin\tauri.cmd dev
