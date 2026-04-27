@echo off
set TABGROUP_HOVER_DETECTOR_VERBOSE=1
echo Verbose logs: %LOCALAPPDATA%\TabGroupShortcut\logs\hover-detector.log
echo Screenshots: %LOCALAPPDATA%\TabGroupShortcut\diagnostics\
:loop
timeout /t 5 /nobreak >nul
.\target\release\hover-detector.exe
goto loop
