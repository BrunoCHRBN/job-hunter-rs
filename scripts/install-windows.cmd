@echo off
setlocal
title Job Hunter RS - Instalador Windows

powershell.exe -NoLogo -NoProfile -ExecutionPolicy Bypass -File "%~dp0install-windows.ps1" %*
set "EXIT_CODE=%ERRORLEVEL%"

if not "%EXIT_CODE%"=="0" (
  echo.
  echo A instalacao terminou com erro. Leia a mensagem acima.
)

exit /b %EXIT_CODE%
