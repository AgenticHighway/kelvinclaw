@echo off
setlocal
powershell -NoLogo -NoProfile -ExecutionPolicy Bypass -File "%~dp0kelvin-tui.ps1" %*
exit /b %ERRORLEVEL%
