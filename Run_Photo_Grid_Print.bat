@echo off
title Photo Grid Print (Rust)
cd /d "%~dp0"
if "%~1"=="" (
    photo_grid_print.exe
) else (
    photo_grid_print.exe --input %*
)
pause
