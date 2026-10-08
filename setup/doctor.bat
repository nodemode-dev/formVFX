@echo off
setlocal
cd /d "%~dp0.."
echo [formvfx] doctor · windows
where node >nul 2>nul && ( for /f %%V in ('node -v') do echo   ok     node %%V ) || echo   FAIL   node · Node 20.19+ is required
where npm >nul 2>nul && echo   ok     npm || echo   FAIL   npm
py -3 -c "import sys; raise SystemExit(0 if sys.version_info >= (3, 12) else 1)" >nul 2>nul && echo   ok     python || echo   FAIL   python · Python 3.12+ is required
where cargo >nul 2>nul && echo   ok     rust || echo   FAIL   rust · the compositing engine needs Rust 1.95+
if exist backend\venv\Scripts\python.exe (
  backend\venv\Scripts\python.exe -c "import fvx_core" >nul 2>nul && echo   ok     compositing engine || echo   FAIL   compositing engine · run formvfx.bat install-native
) else ( echo   warn   engine venv · run formvfx.bat install )
if exist frontend\node_modules ( echo   ok     interface node_modules ) else ( echo   warn   interface deps · run formvfx.bat install )
for %%A in (claude codex cursor-agent) do ( where %%A >nul 2>nul && echo   ok     agent %%A || echo   warn   agent %%A · not on PATH, optional )
