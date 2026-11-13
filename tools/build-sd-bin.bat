@echo off
setlocal
if "%~1"=="" (
    echo Usage: tools\build-sd-bin.bat ^<board^> [output-file]
    echo Example: tools\build-sd-bin.bat zorro
    exit /b 1
)
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0build-sd-bin.ps1" %*
exit /b %ERRORLEVEL%
