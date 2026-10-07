@echo off
setlocal
cd /d "%~dp0.."
for %%P in (frontend\dist backend\build backend\dist-engine src-tauri\target src-tauri\gen native\fvx-core\target) do if exist %%P rmdir /s /q %%P
for /d /r backend %%D in (__pycache__) do if exist "%%D" rmdir /s /q "%%D"
del /q src-tauri\binaries\formvfx-engine-* 2>nul
if /i "%~1"=="--all" (
  if exist frontend\node_modules rmdir /s /q frontend\node_modules
  if exist backend\venv rmdir /s /q backend\venv
)
echo [formvfx] clean
