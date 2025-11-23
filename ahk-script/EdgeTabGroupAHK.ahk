#Requires AutoHotkey v2.0
#SingleInstance Force

; Configuration
colorCheckRadius := 2    ; Radius in pixels to check for hover detection
targetColors := [0x779FF8, 0xE06AB7, 0xC78BD9, 0xB497FE, 0x5987B9, 0x65B1B6, 0xD59367, 0xBCA359, 0x83817E, 0x7BA0FD, 0xDB6ABA, 0xC48BDD, 0xB298FF, 0x5E87BC, 0x6DB1B7, 0xD19262, 0xBAA351, 0x83817E]  ; Colors to check for

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
    if (mouseControl != "Intermediate D3D Window1")
        return false
        
    ; Check for target colors
    return HasTargetColorInRadius(mouseX, mouseY, colorCheckRadius)
}

; Only activate when in Edge AND all conditions are met
#HotIf WinActive("ahk_exe msedge.exe") and IsWithinTargetArea()

; Middle mouse button remap
MButton::Send "!+w"

#HotIf  ; End the context sensitivity
