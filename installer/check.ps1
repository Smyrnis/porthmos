param(
  [Parameter(Mandatory)][string]$Setup,
  [Parameter(Mandatory)][string]$Version
)
$ErrorActionPreference = 'Stop'

$script = Join-Path $PSScriptRoot 'porthmos-gui.iss'
$appId = (Select-String -Path $script -Pattern '^AppId=\{(\{[0-9A-Fa-f-]+\})').Matches[0].Groups[1].Value
$uninstallKey = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\${appId}_is1"
$shortcut = Join-Path ([Environment]::GetFolderPath('Programs')) 'Porthmos.lnk'
$work = Join-Path ([IO.Path]::GetTempPath()) "porthmos-installer-check-$PID"
$dir = Join-Path $work 'Porthmos'
New-Item -ItemType Directory -Force -Path $work | Out-Null

function Install-Silently {
  $log = Join-Path $work 'setup.log'
  $run = Start-Process -FilePath $Setup -ArgumentList '/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART', "/DIR=`"$dir`"", "/LOG=`"$log`"" -Wait -PassThru
  if ($run.ExitCode -ne 0) { throw "the installer exited with $($run.ExitCode)" }
}

function Wait-Until([scriptblock]$condition, [string]$what) {
  $deadline = (Get-Date).AddSeconds(90)
  while (-not (& $condition)) {
    if ((Get-Date) -gt $deadline) { throw "timed out waiting for $what" }
    Start-Sleep -Milliseconds 500
  }
}

try {
  Install-Silently
  $exe = Join-Path $dir 'porthmos-gui.exe'
  foreach ($name in 'porthmos-gui.exe', 'LICENSE', 'unins000.exe') {
    if (-not (Test-Path (Join-Path $dir $name))) { throw "$name was not installed" }
  }
  $embedded = (Get-Item $exe).VersionInfo.ProductVersion
  if ($embedded -ne $Version) { throw "the installed program carries version '$embedded', expected $Version" }
  if (-not (Test-Path $shortcut)) { throw 'the Start Menu shortcut is missing' }
  $registered = Get-ItemProperty $uninstallKey
  if ($registered.DisplayVersion -ne $Version) { throw "the uninstall entry says '$($registered.DisplayVersion)', expected $Version" }

  Set-Content (Join-Path $dir 'porthmos-gui.exe.old') 'left by an update'
  Set-Content (Join-Path $dir '.porthmos.update.1') 'left by an update'
  Install-Silently
  if (-not (Test-Path $exe)) { throw 'the program is missing after installing over itself' }

  $uninstaller = Join-Path $dir 'unins000.exe'
  $run = Start-Process -FilePath $uninstaller -ArgumentList '/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART' -Wait -PassThru
  if ($run.ExitCode -ne 0) { throw "the uninstaller exited with $($run.ExitCode)" }
  Wait-Until { -not (Test-Path $dir) } 'the install folder to be removed (update leftovers must go too)'
  if (Test-Path $shortcut) { throw 'the Start Menu shortcut is still there after uninstalling' }
  if (Test-Path $uninstallKey) { throw 'the uninstall entry is still there after uninstalling' }
  Write-Output 'installed, upgraded in place and removed cleanly'
} finally {
  Remove-Item -Recurse -Force -ErrorAction SilentlyContinue $work
}
