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
; 업데이트 시 실행 중인 LogiFlow 를 알아서 종료. Restart Manager(CloseApplications) 로 먼저 시도하고,
; 트레이 상주라 완전히 안 닫히는 경우를 대비해 [Code] PrepareToInstall 에서 taskkill 로 강제 종료한다.
; (AppMutex 는 '닫아주세요' 수동 프롬프트를 띄우므로 쓰지 않는다.)
CloseApplications=yes
CloseApplicationsFilter=*.exe
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

[Code]
procedure KillRunningLogiFlow();
var
  ResultCode: Integer;
begin
  { 실행 중인 LogiFlow 트레이 앱을 강제 종료(단일 인스턴스 뮤텍스도 함께 해제됨). }
  Exec(ExpandConstant('{sys}\taskkill.exe'), '/F /IM LogiFlow.exe /T', '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
  Sleep(500);
end;

{ 설치(파일 복사) 직전에 호출된다 — 이 시점에 종료해야 파일 잠금 없이 덮어쓴다. }
function PrepareToInstall(var NeedsRestart: Boolean): String;
begin
  KillRunningLogiFlow();
  Result := '';
end;

{ 제거 시작 시에도 실행 중이면 종료. }
function InitializeUninstall(): Boolean;
var
  ResultCode: Integer;
begin
  Exec(ExpandConstant('{sys}\taskkill.exe'), '/F /IM LogiFlow.exe /T', '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
  Sleep(500);
  Result := True;
end;
