; LogiFlow per-user 설치 스크립트 (Inno Setup). LocalDex 방식 참고.
; 설정은 %APPDATA%\LogiFlow 에 저장되므로 설치·제거·업데이트 시 보존된다.
#ifndef AppVersion
  #define AppVersion "0.4.0"
#endif
#ifndef PackageDir
  #define PackageDir "..\..\dist\LogiFlow"
#endif
#ifndef OutputDir
  #define OutputDir "..\..\dist"
#endif

[Setup]
AppId={{B1F3C2A4-7D6E-4C58-9A21-3F0E8C4D5B60}
AppName=LogiFlow
AppVersion={#AppVersion}
AppPublisher=happy4ed
AppPublisherURL=https://github.com/happy4ed/LogiFlow
AppSupportURL=https://github.com/happy4ed/LogiFlow/issues
AppUpdatesURL=https://github.com/happy4ed/LogiFlow/releases
DefaultDirName={localappdata}\Programs\LogiFlow
DefaultGroupName=LogiFlow
PrivilegesRequired=lowest
MinVersion=10.0
OutputDir={#OutputDir}
OutputBaseFilename=LogiFlow-v{#AppVersion}-setup
SetupIconFile=..\..\LogiFlow\Resources\logiflow.ico
UninstallDisplayIcon={app}\LogiFlow.exe
UninstallDisplayName=LogiFlow
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
DisableProgramGroupPage=yes
AppMutex=LogiFlow-A2E1C4D6-8B3F-4A91-9C72-1E5D7F0A2B34
CloseApplications=yes
RestartApplications=no

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"
Name: "korean"; MessagesFile: "compiler:Languages\Korean.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
Source: "{#PackageDir}\*"; DestDir: "{app}"; Excludes: "logs\*,*.pdb"; Flags: ignoreversion recursesubdirs createallsubdirs

[Icons]
Name: "{group}\LogiFlow"; Filename: "{app}\LogiFlow.exe"; WorkingDir: "{app}"
Name: "{autodesktop}\LogiFlow"; Filename: "{app}\LogiFlow.exe"; WorkingDir: "{app}"; Tasks: desktopicon

[Run]
Filename: "{app}\LogiFlow.exe"; Description: "{cm:LaunchProgram,LogiFlow}"; Flags: nowait postinstall skipifsilent
