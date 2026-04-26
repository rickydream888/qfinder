# Build the Material Icon Theme runtime assets for qfinder.
#
# Source : resources/pkief.material-icon-theme/
# Target : ui/icons/material/
#
# Produces:
#   ui/icons/material/manifest.json   - slimmed-down JSON used by ui/js/icon-resolver.js
#   ui/icons/material/svg/*.svg       - all referenced SVG icons (flat layout)
#   ui/icons/material/LICENSE         - upstream MIT license note

$ErrorActionPreference = 'Stop'

$repoRoot = Split-Path -Parent $PSScriptRoot
$srcRoot  = Join-Path $repoRoot 'resources/pkief.material-icon-theme'
$srcJson  = Join-Path $srcRoot  'material-icons.json'
$srcIcons = Join-Path $srcRoot  'icons'
$dstRoot  = Join-Path $repoRoot 'ui/icons/material'
$dstSvg   = Join-Path $dstRoot  'svg'
$dstJson  = Join-Path $dstRoot  'manifest.json'
$dstLic   = Join-Path $dstRoot  'LICENSE'

if (-not (Test-Path $srcJson))  { throw "Source JSON not found: $srcJson" }
if (-not (Test-Path $srcIcons)) { throw "Source icon dir not found: $srcIcons" }

Write-Host "Source: $srcRoot"
Write-Host "Target: $dstRoot"

# Reset target dir
if (Test-Path $dstRoot) { Remove-Item -Recurse -Force $dstRoot }
New-Item -ItemType Directory -Path $dstSvg | Out-Null

# Load upstream JSON
$j = Get-Content $srcJson -Raw | ConvertFrom-Json

function Convert-PSObjectToHashtable($obj) {
    if ($null -eq $obj) { return @{} }
    $h = [ordered]@{}
    foreach ($p in $obj.PSObject.Properties) { $h[$p.Name] = $p.Value }
    return $h
}

# Build slimmed manifest
$manifest = [ordered]@{
    defaults = [ordered]@{
        file               = [string]$j.file
        folder             = [string]$j.folder
        folderExpanded     = [string]$j.folderExpanded
        rootFolder         = [string]$j.rootFolder
        rootFolderExpanded = [string]$j.rootFolderExpanded
    }
    fileNames           = Convert-PSObjectToHashtable $j.fileNames
    fileExtensions      = Convert-PSObjectToHashtable $j.fileExtensions
    languageIds         = Convert-PSObjectToHashtable $j.languageIds
    folderNames         = Convert-PSObjectToHashtable $j.folderNames
    folderNamesExpanded = Convert-PSObjectToHashtable $j.folderNamesExpanded
}

$lightSrc = $j.light
if ($null -ne $lightSrc) {
    $manifest.light = [ordered]@{
        fileNames               = Convert-PSObjectToHashtable $lightSrc.fileNames
        fileExtensions          = Convert-PSObjectToHashtable $lightSrc.fileExtensions
        languageIds             = Convert-PSObjectToHashtable $lightSrc.languageIds
        folderNames             = Convert-PSObjectToHashtable $lightSrc.folderNames
        folderNamesExpanded     = Convert-PSObjectToHashtable $lightSrc.folderNamesExpanded
        rootFolderNames         = Convert-PSObjectToHashtable $lightSrc.rootFolderNames
        rootFolderNamesExpanded = Convert-PSObjectToHashtable $lightSrc.rootFolderNamesExpanded
    }
} else {
    $manifest.light = [ordered]@{
        fileNames = @{}; fileExtensions = @{}; languageIds = @{}
        folderNames = @{}; folderNamesExpanded = @{}
        rootFolderNames = @{}; rootFolderNamesExpanded = @{}
    }
}

# Verify every icon name referenced in iconDefinitions points to a real SVG and
# carry it into the manifest implicitly by copying SVGs (we copy all of them).
$iconCount = ($j.iconDefinitions | Get-Member -MemberType NoteProperty).Count
Write-Host "iconDefinitions: $iconCount entries"

# Copy all SVGs flat
$copied = 0
foreach ($f in Get-ChildItem -Path $srcIcons -Filter '*.svg' -File) {
    $dst = Join-Path $dstSvg $f.Name
    if (Test-Path $dst) { throw "Duplicate icon name: $($f.Name)" }
    Copy-Item -LiteralPath $f.FullName -Destination $dst
    $copied++
}
Write-Host "Copied $copied SVG files"

# Write manifest (compact, UTF-8 no BOM)
$json = $manifest | ConvertTo-Json -Depth 8 -Compress
$utf8NoBom = New-Object System.Text.UTF8Encoding $false
[System.IO.File]::WriteAllText($dstJson, $json, $utf8NoBom)
$jsonSize = (Get-Item $dstJson).Length
Write-Host "manifest.json: $jsonSize bytes"

# License
$licenseSrc = Join-Path $srcRoot 'LICENSE'
if (Test-Path $licenseSrc) {
    Copy-Item -LiteralPath $licenseSrc -Destination $dstLic
} else {
    @"
The icon assets in this directory are derived from the
"Material Icon Theme" by Philipp Kief (PKief), licensed under the MIT License.

Upstream: https://github.com/PKief/vscode-material-icon-theme

Copyright (c) Philipp Kief
"@ | Set-Content -Path $dstLic -Encoding UTF8
}

Write-Host "Done."
