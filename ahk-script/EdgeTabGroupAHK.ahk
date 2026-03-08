#Requires AutoHotkey v2.0
#SingleInstance Force

; Configuration
closeGroupShortcut := "!+w" ; Alt+Shift+W
hoverDetectorExe := ""

ResolveHoverDetectorExe() {
    global hoverDetectorExe

    if (hoverDetectorExe != "" && FileExist(hoverDetectorExe)) {
        return hoverDetectorExe
    }

    envOverride := Trim(EnvGet("TABGROUP_HOVER_DETECTOR_EXE"))
    if (envOverride != "" && FileExist(envOverride)) {
        hoverDetectorExe := envOverride
        return hoverDetectorExe
    }

    candidates := [
        A_ScriptDir "\..\hover-detector\target\release\hover-detector.exe",
        A_ScriptDir "\hover-detector.exe",
        A_WorkingDir "\hover-detector\target\release\hover-detector.exe"
    ]

    for candidate in candidates {
        if FileExist(candidate) {
            hoverDetectorExe := candidate
            return hoverDetectorExe
        }
    }

    hoverDetectorExe := ""
    return ""
}

GetHoveredGroupIndex() {
    detectorExe := ResolveHoverDetectorExe()
    if (detectorExe = "") {
        return 0
    }

    outFile := A_Temp "\tabgroup_hover_" DllCall("GetCurrentProcessId") "_" A_TickCount ".txt"
    cmd := '"' A_ComSpec '" /d /q /c ""' detectorExe '" > "' outFile '" 2>nul"'

    try {
        RunWait(cmd, , "Hide")

        if FileExist(outFile) {
            output := Trim(FileRead(outFile, "UTF-8"))
            FileDelete(outFile)
            if RegExMatch(output, "^\d+$") {
                return Integer(output)
            }
        }
    } catch {
        ; Ignore detector execution errors and fall back to normal middle click.
    }

    if FileExist(outFile) {
        try FileDelete(outFile)
    }

    return 0
}

; Only activate when Edge is active.
#HotIf WinActive("ahk_exe msedge.exe")

; Middle mouse button remap
$MButton::
{
    global closeGroupShortcut

    if (GetHoveredGroupIndex() > 0) {
        Send closeGroupShortcut
    } else {
        ; Not on a tab-group tag: keep default middle-click behavior.
        Send "{Blind}{MButton}"
    }
}

#HotIf  ; End the context sensitivity
