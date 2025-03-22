#Requires AutoHotkey v2.0
#SingleInstance Force

; Configuration
colorCheckRadius := 2    ; Radius in pixels to check for hover detection
targetColors := [0x7AA1FA, 0xDC6AB8, 0xC58CDB, 0xB299FF, 0x5D88BA, 0x6CB2B7, 0xD29265, 0xBBA356, 0x84817E]  ; Colors to check for

; Function to check if a color matches any target color
IsTargetColor(color) {
    for targetColor in targetColors {
        if (color = targetColor)
            return true
    }
    return false
}

; Function to check if any pixel within radius matches target colors
HasTargetColorInRadius(x, y, radius) {
    offsetY := -radius
    while offsetY <= radius {
        offsetX := -radius
        while offsetX <= radius {
            try {
                color := PixelGetColor(x + offsetX, y + offsetY)
                if IsTargetColor(color)
                    return true
            }
            offsetX++
        }
        offsetY++
    }
    return false
}

; Function to check all conditions
IsWithinTargetArea() {
    ; Get mouse position and control info
    MouseGetPos(&mouseX, &mouseY, , &mouseControl)
    
    ; Check if mouse is over the correct control
    if (mouseControl != "Intermediate D3D Window2")
        return false
        
    ; Check for target colors
    return HasTargetColorInRadius(mouseX, mouseY, colorCheckRadius)
}

; Only activate when in Edge AND all conditions are met
#HotIf WinActive("ahk_exe msedge.exe") and IsWithinTargetArea()

; Middle mouse button remap
MButton::Send "!+w"

#HotIf  ; End the context sensitivity
