#ifndef AppVersion
  #error AppVersion must be supplied by scripts/package.ps1
#endif
#ifndef SourceExe
  #error SourceExe must be supplied by scripts/package.ps1
#endif
#ifndef OutputPath
  #error OutputPath must be supplied by scripts/package.ps1
#endif

[Setup]
AppId={{EA4B331C-38DE-4F1B-97DE-3BB671F82434}
AppName=Githly
AppVersion={#AppVersion}
AppPublisher=Hunlongyu
AppPublisherURL=https://github.com/Hunlongyu/githly
DefaultDirName={localappdata}\Programs\Githly
DefaultGroupName=Githly
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0
OutputDir={#OutputPath}
OutputBaseFilename=Githly-{#AppVersion}-windows-x64-setup
SetupIconFile=..\assets\branding\githly.ico
UninstallDisplayIcon={app}\Githly.exe
Compression=lzma2
SolidCompression=yes
CloseApplications=yes
RestartApplications=no
WizardStyle=modern

[Files]
Source: "{#SourceExe}"; DestDir: "{app}"; DestName: "Githly.exe"; Flags: ignoreversion
Source: "..\vendor\gpui-component\LICENSE-APACHE"; DestDir: "{app}"; DestName: "GPUI-Component-LICENSE.txt"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\Githly"; Filename: "{app}\Githly.exe"
Name: "{autodesktop}\Githly"; Filename: "{app}\Githly.exe"; WorkingDir: "{app}"

[Run]
Filename: "{app}\Githly.exe"; Description: "Launch Githly"; Flags: nowait postinstall skipifsilent

; Configuration in Roaming AppData is deliberately retained on uninstall.
