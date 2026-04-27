[CmdletBinding()]
param()

$ErrorActionPreference = "Stop"

function Resolve-RequiredFile {
    param(
        [string]$Path,
        [string]$Description
    )

    if (-not (Test-Path -LiteralPath $Path)) {
        throw "Missing $Description at $Path"
    }

    return (Resolve-Path -LiteralPath $Path).Path
}

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$settings = Get-Content -Raw -LiteralPath (Join-Path $repoRoot "installer\settings.json") | ConvertFrom-Json
$manifest = Get-Content -Raw -LiteralPath (Join-Path $repoRoot "manifest.json") | ConvertFrom-Json

$hostExe = Resolve-RequiredFile -Description "native-host.exe" -Path (Join-Path $repoRoot "native-host\target\release\native-host.exe")
$detectorExe = Resolve-RequiredFile -Description "hover-detector.exe" -Path (Join-Path $repoRoot "hover-detector\target\release\hover-detector.exe")

$outputRoot = Join-Path $repoRoot "dist\windows-companion"
$packageName = "tab-group-shortcut-windows-companion-$($manifest.version)"
$packageDir = Join-Path $outputRoot $packageName
$zipPath = Join-Path $outputRoot "$packageName.zip"

if (Test-Path -LiteralPath $packageDir) {
    Remove-Item -LiteralPath $packageDir -Recurse -Force
}

if (Test-Path -LiteralPath $zipPath) {
    Remove-Item -LiteralPath $zipPath -Force
}

New-Item -ItemType Directory -Path $packageDir -Force | Out-Null
New-Item -ItemType Directory -Path (Join-Path $packageDir "installer") -Force | Out-Null
New-Item -ItemType Directory -Path (Join-Path $packageDir "docs") -Force | Out-Null

Copy-Item -LiteralPath (Join-Path $repoRoot "install.ps1") -Destination (Join-Path $packageDir "install.ps1") -Force
Copy-Item -LiteralPath (Join-Path $repoRoot "uninstall.ps1") -Destination (Join-Path $packageDir "uninstall.ps1") -Force
Copy-Item -LiteralPath (Join-Path $repoRoot "installer\settings.json") -Destination (Join-Path $packageDir "installer\settings.json") -Force
Copy-Item -LiteralPath (Join-Path $repoRoot "installer\native-messaging-host.template.json") -Destination (Join-Path $packageDir "installer\native-messaging-host.template.json") -Force
Copy-Item -LiteralPath (Join-Path $repoRoot "docs\WINDOWS_SETUP.md") -Destination (Join-Path $packageDir "docs\WINDOWS_SETUP.md") -Force
Copy-Item -LiteralPath (Join-Path $repoRoot "docs\ATTRIBUTION.md") -Destination (Join-Path $packageDir "docs\ATTRIBUTION.md") -Force
Copy-Item -LiteralPath $hostExe -Destination (Join-Path $packageDir "native-host.exe") -Force
Copy-Item -LiteralPath $detectorExe -Destination (Join-Path $packageDir "hover-detector.exe") -Force

$ahkScript = Join-Path $repoRoot "ahk-script\EdgeTabGroupAHK.ahk"
if (Test-Path -LiteralPath $ahkScript) {
    New-Item -ItemType Directory -Path (Join-Path $packageDir "ahk-script") -Force | Out-Null
    Copy-Item -LiteralPath $ahkScript -Destination (Join-Path $packageDir "ahk-script\EdgeTabGroupAHK.ahk") -Force
}

$ahkExe = Join-Path $repoRoot "ahk-script\EdgeTabGroupAHK.exe"
if (Test-Path -LiteralPath $ahkExe) {
    New-Item -ItemType Directory -Path (Join-Path $packageDir "ahk-script") -Force | Out-Null
    Copy-Item -LiteralPath $ahkExe -Destination (Join-Path $packageDir "ahk-script\EdgeTabGroupAHK.exe") -Force
}

Compress-Archive -Path (Join-Path $packageDir "*") -DestinationPath $zipPath -Force

Write-Host "Created Windows companion package: $zipPath"
Write-Host "Default development extension ID: $($settings.developmentExtensionId)"
