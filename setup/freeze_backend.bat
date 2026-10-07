@echo off
setlocal
cd /d "%~dp0.."
set "ROOT=%cd%"
if not exist backend\venv\Scripts\activate.bat ( echo [formvfx] engine venv missing, run formvfx.bat install & exit /b 1 )
call backend\venv\Scripts\activate.bat
python -c "import PyInstaller" >nul 2>nul || ( echo [formvfx] PyInstaller missing, run formvfx.bat install-desktop & exit /b 1 )
python -c "import fvx_core" >nul 2>nul || ( echo [formvfx] the compositing core is not built, run formvfx.bat install-native & exit /b 1 )
if defined FORMVFX_TARGET ( set "TRIPLE=%FORMVFX_TARGET%" ) else ( for /f "tokens=2" %%t in ('rustc -vV ^| findstr /b "host:"') do set "TRIPLE=%%t" )
if not defined TRIPLE ( echo [formvfx] rustc not found, install Rust or set FORMVFX_TARGET & exit /b 1 )
if not exist frontend\dist\index.html call setup\build_frontend.bat
cd backend
if exist build\pyinstaller rmdir /s /q build\pyinstaller
if exist dist-engine rmdir /s /q dist-engine
pyinstaller --noconfirm --clean --onefile --name formvfx-engine --distpath dist-engine --workpath build\pyinstaller --specpath build\pyinstaller --add-data "%ROOT%\frontend\dist;frontend_dist" --add-data "%ROOT%\backend\app\agent\templates;agent_templates" --add-data "%ROOT%\.claude\skills;skills" --collect-all fvx_core --collect-submodules app --collect-submodules uvicorn --collect-submodules winpty "%ROOT%\backend\formvfx_engine.py" || exit /b 1
if not exist "%ROOT%\src-tauri\binaries" mkdir "%ROOT%\src-tauri\binaries"
copy /y dist-engine\formvfx-engine.exe "%ROOT%\src-tauri\binaries\formvfx-engine-%TRIPLE%.exe" >nul
"%ROOT%\src-tauri\binaries\formvfx-engine-%TRIPLE%.exe" --selfcheck
