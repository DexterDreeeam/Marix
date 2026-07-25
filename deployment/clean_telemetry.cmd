@echo off
"%SystemRoot%\System32\WindowsPowerShell\v1.0\powershell.exe" -NoLogo -NoProfile -NonInteractive -ExecutionPolicy Bypass -File "%~dp0win\clean_telemetry.ps1" %*
set "exitCode=%errorlevel%"
pause
exit /b %exitCode%
