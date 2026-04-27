[CmdletBinding()]
param()

$ErrorActionPreference = "Stop"

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$manifestPath = Join-Path $repoRoot "manifest.json"
$manifest = Get-Content -Raw -LiteralPath $manifestPath | ConvertFrom-Json
$manifest.PSObject.Properties.Remove("key")

$outputRoot = Join-Path $repoRoot "dist\edge-store"
$packageName = "edge-tab-group-shortcut-$($manifest.version)"
$packageDir = Join-Path $outputRoot $packageName
$zipPath = Join-Path $outputRoot "$packageName.zip"

if (Test-Path -LiteralPath $packageDir) {
    Remove-Item -LiteralPath $packageDir -Recurse -Force
}

if (Test-Path -LiteralPath $zipPath) {
    Remove-Item -LiteralPath $zipPath -Force
}

New-Item -ItemType Directory -Path $packageDir -Force | Out-Null

$filesToCopy = @(
    "background.js",
    "help.html",
    "icon48.png",
    "icon128.png"
)

foreach ($relativePath in $filesToCopy) {
    Copy-Item -LiteralPath (Join-Path $repoRoot $relativePath) -Destination (Join-Path $packageDir $relativePath) -Force
}

$manifest | ConvertTo-Json -Depth 10 | Set-Content -LiteralPath (Join-Path $packageDir "manifest.json") -Encoding UTF8
Compress-Archive -Path (Join-Path $packageDir "*") -DestinationPath $zipPath -Force

Write-Host "Created Edge Store package: $zipPath"
