@echo off
setlocal
powershell -NoLogo -NoProfile -ExecutionPolicy Bypass -File "%~dp0kelvin-gateway.ps1" %*
exit /b %ERRORLEVEL%
