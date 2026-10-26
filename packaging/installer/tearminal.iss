; Instalador de TEArminal (Inno Setup 6). Se compila con packaging/package.ps1:
;   ISCC /DAppVersion=0.7.0 /DStageDir=..\dist\tearminal packaging\installer\tearminal.iss
; Instalación por usuario (sin permisos de administrador) en %LOCALAPPDATA%\Programs\TEArminal.
#ifndef AppVersion
  #define AppVersion "0.0.0"
#endif
#ifndef StageDir
  #define StageDir "..\..\dist\tearminal"
#endif

[Setup]
; GUID fijo: identifica la aplicación en cada actualización y en la desinstalación. NO cambiarlo.
AppId={{E31245EC-A467-44E0-AC9B-A1DD44243430}
AppName=TEArminal
AppVersion={#AppVersion}
AppPublisher=TEArminal Team
AppPublisherURL=https://github.com/GonzhaI/TEArminal
AppSupportURL=https://github.com/GonzhaI/TEArminal/issues
DefaultDirName={autopf}\TEArminal
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
; ConPTY exige Windows 10 versión 1809 (build 17763)
MinVersion=10.0.17763
OutputDir=..\..\dist
OutputBaseFilename=tearminal-setup-{#AppVersion}
SetupIconFile=..\..\assets\icon.ico
UninstallDisplayIcon={app}\tearminal.exe
WizardStyle=modern
Compression=lzma2/max
SolidCompression=yes
CloseApplications=yes
RestartApplications=no

[Languages]
Name: "spanish"; MessagesFile: "compiler:Languages\Spanish.isl"
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
; tearminal.toml NO se instala: TEArminal genera la configuración por defecto en %APPDATA%\tearminal y no pisamos la del usuario
Source: "{#StageDir}\tearminal.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#StageDir}\bin\*"; DestDir: "{app}\bin"; Flags: ignoreversion recursesubdirs createallsubdirs
Source: "{#StageDir}\licenses\*"; DestDir: "{app}\licenses"; Flags: ignoreversion recursesubdirs createallsubdirs

[Icons]
Name: "{autoprograms}\TEArminal"; Filename: "{app}\tearminal.exe"
Name: "{autodesktop}\TEArminal"; Filename: "{app}\tearminal.exe"; Tasks: desktopicon

[Run]
Filename: "{app}\tearminal.exe"; Description: "{cm:LaunchProgram,TEArminal}"; Flags: nowait postinstall skipifsilent
