[CmdletBinding()]
param(
    [ValidateSet("development", "published")]
    [string]$Channel = "development",

    [string]$ExtensionId
)

$ErrorActionPreference = "Stop"

function Resolve-SourceFile {
    param(
        [string[]]$Candidates,
        [string]$Description
    )

    foreach ($candidate in $Candidates) {
        if (Test-Path -LiteralPath $candidate) {
            return (Resolve-Path -LiteralPath $candidate).Path
        }
    }

    throw "Could not find $Description. Build the release binaries first or place the packaged binaries next to install.ps1."
}

$repoRoot = $PSScriptRoot
$settingsPath = Join-Path $repoRoot "installer\settings.json"
$settings = Get-Content -Raw -LiteralPath $settingsPath | ConvertFrom-Json

if ([string]::IsNullOrWhiteSpace($ExtensionId)) {
    if ($Channel -eq "published") {
        if ([string]::IsNullOrWhiteSpace($settings.publishedExtensionId)) {
            throw "No published extension ID is configured. Pass -ExtensionId or update installer/settings.json after the first Edge Add-ons submission."
        }

        $ExtensionId = $settings.publishedExtensionId
    }
    else {
        $ExtensionId = $settings.developmentExtensionId
    }
}

$installRoot = Join-Path $env:LOCALAPPDATA $settings.installDirectoryName
$hostTargetPath = Join-Path $installRoot "native-host.exe"
$detectorTargetPath = Join-Path $installRoot "hover-detector.exe"
$manifestTargetPath = Join-Path $installRoot "com.tabgroup.shortcut.json"

$hostSourcePath = Resolve-SourceFile -Description "native-host.exe" -Candidates @(
    (Join-Path $repoRoot "native-host.exe"),
    (Join-Path $repoRoot "native-host\target\release\native-host.exe")
)

$detectorSourcePath = Resolve-SourceFile -Description "hover-detector.exe" -Candidates @(
    (Join-Path $repoRoot "hover-detector.exe"),
    (Join-Path $repoRoot "hover-detector\target\release\hover-detector.exe")
)

New-Item -ItemType Directory -Path $installRoot -Force | Out-Null
New-Item -ItemType Directory -Path (Join-Path $installRoot "logs") -Force | Out-Null
New-Item -ItemType Directory -Path (Join-Path $installRoot "diagnostics") -Force | Out-Null

Copy-Item -LiteralPath $hostSourcePath -Destination $hostTargetPath -Force
Copy-Item -LiteralPath $detectorSourcePath -Destination $detectorTargetPath -Force

$manifestTemplatePath = Join-Path $repoRoot "installer\native-messaging-host.template.json"
$manifest = Get-Content -Raw -LiteralPath $manifestTemplatePath | ConvertFrom-Json
$manifest.path = $hostTargetPath
$manifest.allowed_origins = @("chrome-extension://$ExtensionId/")
$manifest | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath $manifestTargetPath -Encoding UTF8

$registryPath = "Software\Microsoft\Edge\NativeMessagingHosts\com.tabgroup.shortcut"
$registryKey = [Microsoft.Win32.Registry]::CurrentUser.CreateSubKey($registryPath)
if (-not $registryKey) {
    throw "Could not create the Edge native messaging registry key."
}

$registryKey.SetValue("", $manifestTargetPath, [Microsoft.Win32.RegistryValueKind]::String)
$registryKey.Dispose()

Write-Host "Installed TabGroup Keyboard Shortcuts companion files to $installRoot"
Write-Host "Registered native messaging host for extension ID $ExtensionId"
