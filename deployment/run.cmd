@echo off
"%SystemRoot%\System32\WindowsPowerShell\v1.0\powershell.exe" -NoLogo -NoProfile -NonInteractive -ExecutionPolicy Bypass -File "%~dp0win\run.ps1" %*
set "exitCode=%errorlevel%"
if not "%exitCode%"=="0" (
  echo.
  echo Deployment failed with exit code %exitCode%.
  pause
)
exit /b %exitCode%
