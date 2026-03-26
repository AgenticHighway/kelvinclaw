@echo off
setlocal
powershell -NoLogo -NoProfile -ExecutionPolicy Bypass -File "%~dp0start-gateway.ps1" %*
exit /b %ERRORLEVEL%
