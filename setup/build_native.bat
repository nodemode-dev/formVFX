@echo off
setlocal
cd /d "%~dp0.."
if not exist backend\venv\Scripts\activate.bat ( echo [formvfx] engine venv missing, run formvfx.bat install & exit /b 1 )
call backend\venv\Scripts\activate.bat
where cargo >nul 2>nul || ( echo [formvfx] Rust is required for the compositing engine: https://rustup.rs & exit /b 1 )
set "PROFILE=--release"
for %%A in (%*) do (
  if /i "%%A"=="--dev" set "PROFILE="
)
python -c "import maturin" 2>nul || pip install "maturin>=1.15,<2"
pushd native\fvx-core
maturin develop %PROFILE% || ( popd & exit /b 1 )
popd
python -c "import json, fvx_core; c = json.loads(fvx_core.Engine().capabilities()); print('[formvfx] engine', fvx_core.__version__, 'ready:', c['commands'], 'commands,', c['effects']['total'], 'effects')"
