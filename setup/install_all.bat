@echo off
setlocal
cd /d "%~dp0.."
echo [formvfx] installing the engine service
pushd backend
call oc_install.bat || ( popd & exit /b 1 )
popd
echo [formvfx] building the compositing engine (the first build takes a few minutes)
call setup\build_native.bat %1 || ( echo [formvfx] the compositing core did not build, run formvfx.bat doctor & exit /b 1 )
echo [formvfx] installing the interface
pushd frontend
call oc_install.bat || ( popd & exit /b 1 )
popd
if not exist backend\.env copy backend\.env.example backend\.env >nul
echo [formvfx] done. Boot with formvfx.bat
