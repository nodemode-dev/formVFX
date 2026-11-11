@echo off
setlocal
if not defined FORMVFX_CALLER_DIR set "FORMVFX_CALLER_DIR=%CD%"
cd /d "%~dp0"
set "CMD=%~1"
if "%CMD%"=="" set "CMD=boot"
if /i "%CMD%"=="boot" ( call setup\boot_all.bat & goto :eof )
if /i "%CMD%"=="dev" ( call setup\boot_all.bat & goto :eof )
if /i "%CMD%"=="install" ( call setup\install_all.bat %2 %3 %4 %5 %6 %7 %8 %9 & goto :eof )
if /i "%CMD%"=="install-native" ( call setup\build_native.bat %2 %3 %4 %5 %6 %7 %8 %9 & goto :eof )
if /i "%CMD%"=="install-desktop" ( call setup\install_desktop.bat & goto :eof )
if /i "%CMD%"=="backend" ( call setup\boot_backend.bat & goto :eof )
if /i "%CMD%"=="frontend" ( call setup\boot_frontend.bat & goto :eof )
if /i "%CMD%"=="build" ( call setup\build_frontend.bat & goto :eof )
if /i "%CMD%"=="serve" ( call setup\serve.bat & goto :eof )
if /i "%CMD%"=="freeze" ( call setup\freeze_backend.bat & goto :eof )
if /i "%CMD%"=="desktop" (
  if /i "%~2"=="build" ( call setup\desktop_build.bat %3 %4 %5 %6 %7 %8 %9 ) else ( call setup\desktop_dev.bat )
  goto :eof
)
if /i "%CMD%"=="doctor" ( call setup\doctor.bat & goto :eof )
for %%C in (exec batch commands describe state frame render extensions extension) do if /i "%CMD%"=="%%C" ( call :py setup\fx.py %* & goto :eof )
if /i "%CMD%"=="mcp" ( call :py setup\mcp_stdio.py %2 %3 %4 %5 %6 %7 %8 %9 & goto :eof )
if /i "%CMD%"=="agents" ( call :py setup\agents.py %2 %3 %4 %5 %6 %7 %8 %9 & goto :eof )
if /i "%CMD%"=="counts" ( call :py setup\line_counts.py %2 %3 %4 %5 %6 %7 %8 %9 & goto :eof )
if /i "%CMD%"=="presets" ( call :py setup\text_presets.py %2 %3 %4 %5 %6 %7 %8 %9 & goto :eof )
if /i "%CMD%"=="clean" ( call setup\clean.bat %2 %3 %4 %5 %6 %7 %8 %9 & goto :eof )
if /i "%CMD%"=="help" ( call :usage & exit /b 0 )
echo [formvfx] unknown command: %CMD%
call :usage
exit /b 2
:py
if exist backend\venv\Scripts\python.exe ( backend\venv\Scripts\python.exe %* ) else ( py -3 %* )
exit /b %errorlevel%
:usage
echo formvfx.bat [command]
echo   boot ^| install [--dev] ^| install-native [--dev] ^| install-desktop ^| backend ^| frontend
echo   build ^| serve ^| freeze ^| desktop dev ^| desktop build [TAURI ARGS] ^| doctor
echo   desktop build runs the interface build, the freeze and tauri build; build_app.bat also installs first
echo   exec ID [JSON] ^| batch JSON ^| commands [FILTER] ^| describe ID ^| state ^| frame OUT.png ^| render
echo   mcp ^| agents install^|sync^|status ^| counts ^| presets ^| clean [--all] ^| help
exit /b 0
