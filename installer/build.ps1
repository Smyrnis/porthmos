param(
  [Parameter(Mandatory)][string]$Exe,
  [Parameter(Mandatory)][string]$Version,
  [Parameter(Mandatory)][string]$Out
)
$ErrorActionPreference = 'Stop'

function Find-Compiler {
  $found = Get-Command iscc -ErrorAction SilentlyContinue | Select-Object -First 1 -ExpandProperty Source
  if ($found) { return $found }
  $roots = @($env:ProgramFiles, ${env:ProgramFiles(x86)}, (Join-Path $env:LOCALAPPDATA 'Programs')) | Where-Object { $_ }
  foreach ($root in $roots) {
    $candidate = Join-Path $root 'Inno Setup 6\ISCC.exe'
    if (Test-Path $candidate) { return $candidate }
  }
  return $null
}

$compiler = Find-Compiler
if (-not $compiler) {
  choco install innosetup --no-progress -y
  if ($LASTEXITCODE -ne 0) { throw 'Inno Setup could not be installed' }
  $compiler = Find-Compiler
}
if (-not $compiler) { throw 'Inno Setup was not found after installing it' }

$script = Join-Path $PSScriptRoot 'porthmos-gui.iss'
$exePath = (Resolve-Path $Exe).Path
New-Item -ItemType Directory -Force -Path $Out | Out-Null
$outPath = (Resolve-Path $Out).Path
$numeric = (($Version -split '-')[0]) + '.0'

& $compiler "/DAppVersion=$Version" "/DNumericVersion=$numeric" "/DSourceExe=$exePath" "/DOutputDir=$outPath" "/DOutputName=porthmos-gui-$Version-setup" $script
if ($LASTEXITCODE -ne 0) { throw "iscc failed with exit code $LASTEXITCODE" }

$versioned = Join-Path $outPath "porthmos-gui-$Version-setup.exe"
if (-not (Test-Path $versioned)) { throw "the compiler did not write $versioned" }
Copy-Item $versioned (Join-Path $outPath 'porthmos-gui-setup.exe') -Force
Write-Output "wrote $versioned and porthmos-gui-setup.exe"
