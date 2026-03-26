@echo off
setlocal
powershell -NoLogo -NoProfile -ExecutionPolicy Bypass -File "%~dp0kpm.ps1" %*
exit /b %ERRORLEVEL%
