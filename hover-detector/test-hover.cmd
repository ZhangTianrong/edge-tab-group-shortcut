@echo off
:loop
timeout /t 5 /nobreak >nul
.\target\release\hover-detector.exe
goto loop
