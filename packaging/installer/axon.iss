; Instalador de Axon (Inno Setup 6). Se compila con packaging/package.ps1:
;   ISCC /DAppVersion=0.6.0 /DStageDir=..\dist\axon packaging\installer\axon.iss
; Instalación por usuario (sin permisos de administrador) en %LOCALAPPDATA%\Programs\Axon.
#ifndef AppVersion
  #define AppVersion "0.0.0"
#endif
#ifndef StageDir
  #define StageDir "..\..\dist\axon"
#endif

[Setup]
; GUID fijo: identifica la aplicación en cada actualización y en la desinstalación. NO cambiarlo.
AppId={{E31245EC-A467-44E0-AC9B-A1DD44243430}
AppName=Axon
AppVersion={#AppVersion}
AppPublisher=Axon Team
AppPublisherURL=https://github.com/GonzhaI/axon
AppSupportURL=https://github.com/GonzhaI/axon/issues
DefaultDirName={autopf}\Axon
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
; ConPTY exige Windows 10 versión 1809 (build 17763)
MinVersion=10.0.17763
OutputDir=..\..\dist
OutputBaseFilename=axon-setup-{#AppVersion}
SetupIconFile=..\..\assets\icon.ico
UninstallDisplayIcon={app}\axon.exe
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
; axon.toml NO se instala: Axon genera la configuración por defecto en %APPDATA%\axon y no pisamos la del usuario
Source: "{#StageDir}\axon.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#StageDir}\bin\*"; DestDir: "{app}\bin"; Flags: ignoreversion recursesubdirs createallsubdirs
Source: "{#StageDir}\licenses\*"; DestDir: "{app}\licenses"; Flags: ignoreversion recursesubdirs createallsubdirs

[Icons]
Name: "{autoprograms}\Axon"; Filename: "{app}\axon.exe"
Name: "{autodesktop}\Axon"; Filename: "{app}\axon.exe"; Tasks: desktopicon

[Run]
Filename: "{app}\axon.exe"; Description: "{cm:LaunchProgram,Axon}"; Flags: nowait postinstall skipifsilent
