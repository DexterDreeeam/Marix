@echo off
for %%I in ("%~dp0..") do set "repoRoot=%%~fI"
"%SystemRoot%\System32\WindowsPowerShell\v1.0\powershell.exe" -NoLogo -NoProfile -NonInteractive -ExecutionPolicy Bypass -File "%~dp0win\09-stop-server.ps1" -RepoRoot "%repoRoot%"
exit /b %errorlevel%
