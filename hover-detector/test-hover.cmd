@echo off
set TABGROUP_HOVER_DETECTOR_VERBOSE=1
:loop
timeout /t 5 /nobreak >nul
.\target\release\hover-detector.exe
goto loop
