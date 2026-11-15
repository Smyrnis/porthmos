#ifndef AppVersion
  #error Pass the version with /DAppVersion=...
#endif
#ifndef NumericVersion
  #define NumericVersion "0.0.0.0"
#endif
#ifndef SourceExe
  #error Pass the program with /DSourceExe=...
#endif
#ifndef OutputDir
  #define OutputDir "..\dist"
#endif
#ifndef OutputName
  #define OutputName "porthmos-gui-setup"
#endif

[Setup]
AppId={{C3D7992D-DE8E-426D-BA73-CA4D3332C849}
AppName=Porthmos
AppVersion={#AppVersion}
AppPublisher=Porthmos
AppPublisherURL=https://github.com/Smyrnis/porthmos
AppSupportURL=https://github.com/Smyrnis/porthmos/issues
AppUpdatesURL=https://github.com/Smyrnis/porthmos/releases
DefaultDirName={localappdata}\Programs\Porthmos
DefaultGroupName=Porthmos
DisableProgramGroupPage=yes
DisableWelcomePage=no
DisableDirPage=no
DisableReadyPage=no
DisableFinishedPage=no
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
CloseApplications=yes
RestartApplications=no
LicenseFile=..\LICENSE
SetupIconFile=..\assets\icon\porthmos.ico
UninstallDisplayName=Porthmos
UninstallDisplayIcon={app}\porthmos-gui.exe
VersionInfoVersion={#NumericVersion}
VersionInfoProductName=Porthmos
VersionInfoDescription=Porthmos setup
OutputDir={#OutputDir}
OutputBaseFilename={#OutputName}
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "Create a &desktop shortcut"; GroupDescription: "Shortcuts:"; Flags: unchecked

[Files]
Source: "{#SourceExe}"; DestDir: "{app}"; DestName: "porthmos-gui.exe"; Flags: ignoreversion
Source: "..\LICENSE"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\Porthmos"; Filename: "{app}\porthmos-gui.exe"
Name: "{autodesktop}\Porthmos"; Filename: "{app}\porthmos-gui.exe"; Tasks: desktopicon

[Run]
Filename: "{app}\porthmos-gui.exe"; Description: "Start Porthmos"; Flags: nowait postinstall skipifsilent

[UninstallDelete]
Type: files; Name: "{app}\*.old"
Type: filesandordirs; Name: "{app}\.porthmos.*"
