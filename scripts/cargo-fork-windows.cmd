@echo off
node "%~dp0cargo-fork-windows.cjs" %*
exit /b %errorlevel%
