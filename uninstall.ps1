[CmdletBinding()]
param()

$ErrorActionPreference = "Stop"

$settingsPath = Join-Path $PSScriptRoot "installer\settings.json"
$settings = Get-Content -Raw -LiteralPath $settingsPath | ConvertFrom-Json
$installRoot = Join-Path $env:LOCALAPPDATA $settings.installDirectoryName
$expectedRoot = [System.IO.Path]::GetFullPath((Join-Path $env:LOCALAPPDATA $settings.installDirectoryName))
$resolvedRoot = [System.IO.Path]::GetFullPath($installRoot)

if ($resolvedRoot -ne $expectedRoot) {
    throw "Refusing to remove an unexpected install directory: $resolvedRoot"
}

$registryBase = [Microsoft.Win32.Registry]::CurrentUser
$registryPath = "Software\Microsoft\Edge\NativeMessagingHosts"

try {
    $registryBase.DeleteSubKeyTree("$registryPath\com.tabgroup.shortcut", $false)
} catch {
    if ($_.Exception.Message -notmatch "cannot find") {
        throw
    }
}

if (Test-Path -LiteralPath $installRoot) {
    Remove-Item -LiteralPath $installRoot -Recurse -Force
}

Write-Host "Removed TabGroup Keyboard Shortcuts native companion files."
