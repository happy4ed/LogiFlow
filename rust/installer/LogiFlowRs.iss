; LogiFlow(Windows 기본 판, 0.16.0~) — 사용자 범위 설치(Inno Setup).
; 사용자에게 보이는 이름은 「LogiFlow」. 내부 이름(AppId·설치 폴더 LogiFlowRs·실행 파일 LogiFlowRs.exe)은 rs 시험판과 같게 두어
; 이미 깔린 rs 판은 그 자리에서 업그레이드된다. x64·arm64 실행 파일을 한 설치 파일에 담아 PC 에 맞는 쪽을 깐다.
; 이전 판(C# LogiFlow, AppId {B1F3C2A4-…})이 깔려 있으면 설치 전에 그 제거 프로그램을 조용히 실행한다(사용자 범위라 관리자 승인 없음).
; 이전 판 설정(%APPDATA%\LogiFlow)은 이전 판 제거 프로그램도 지우지 않으며, 이 앱이 처음 실행될 때 이어받는다.
; 이전 판이 남긴 「로그온 전에도 실행」 서비스(관리자 범위)는 앱의 「기존 LogiFlow 정리」가 지운다.
; 제거 때 서비스·워커 사본·예약 작업·방화벽 규칙까지 정리한다(--uninstall-cleanup, 필요할 때만 UAC).
#ifndef AppVersion
  #define AppVersion "0.16.0"
#endif
#ifndef FileVersion
  #define FileVersion "0.16.0.0"
#endif
#ifndef DistDir
  #define DistDir "..\dist"
#endif
#ifndef OutputDir
  #define OutputDir "..\dist"
#endif

[Setup]
AppId={{6F3B2D1A-9C84-4E57-B0A3-7D52E1C9F046}
AppName=LogiFlow
AppVersion={#AppVersion}
AppVerName=LogiFlow {#AppVersion}
VersionInfoVersion={#FileVersion}
AppPublisher=happy4ed
AppPublisherURL=https://github.com/happy4ed/LogiFlow
AppSupportURL=https://github.com/happy4ed/LogiFlow/issues
AppUpdatesURL=https://github.com/happy4ed/LogiFlow/releases
DefaultDirName={localappdata}\Programs\LogiFlowRs
DefaultGroupName=LogiFlow
PrivilegesRequired=lowest
MinVersion=10.0
ArchitecturesAllowed=x64compatible arm64
ArchitecturesInstallIn64BitMode=x64compatible arm64
OutputDir={#OutputDir}
OutputBaseFilename=LogiFlow-v{#AppVersion}-setup
SetupIconFile=..\..\LogiFlow\Resources\logiflow.ico
UninstallDisplayIcon={app}\LogiFlowRs.exe
UninstallDisplayName=LogiFlow
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
DisableProgramGroupPage=yes
CloseApplications=yes
CloseApplicationsFilter=LogiFlowRs.exe
RestartApplications=no

[Languages]
Name: "korean"; MessagesFile: "compiler:Languages\Korean.isl"
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
Source: "{#DistDir}\x64\LogiFlowRs.exe"; DestDir: "{app}"; Flags: ignoreversion; Check: not IsArm64
Source: "{#DistDir}\arm64\LogiFlowRs.exe"; DestDir: "{app}"; Flags: ignoreversion; Check: IsArm64

[Icons]
Name: "{group}\LogiFlow"; Filename: "{app}\LogiFlowRs.exe"; WorkingDir: "{app}"
Name: "{autodesktop}\LogiFlow"; Filename: "{app}\LogiFlowRs.exe"; WorkingDir: "{app}"; Tasks: desktopicon

[Run]
Filename: "{app}\LogiFlowRs.exe"; Description: "{cm:LaunchProgram,LogiFlow}"; Flags: nowait postinstall skipifsilent

[UninstallRun]
; 파일을 지우기 전에 실행된다. 머신 범위에 남은 것이 있으면 그때만 UAC 를 묻는다.
Filename: "{app}\LogiFlowRs.exe"; Parameters: "--uninstall-cleanup"; Flags: waituntilterminated; RunOnceId: "LogiFlowRsCleanup"

[UninstallDelete]
Type: filesandordirs; Name: "{app}\logs"

[Code]
const
  LegacyKey = 'Software\Microsoft\Windows\CurrentVersion\Uninstall\{B1F3C2A4-7D6E-4C58-9A21-3F0E8C4D5B60}_is1';

var
  LegacyFound: Boolean;
  LegacyLeft: Boolean;

function LegacyUninstaller(var Path: String): Boolean;
begin
  Result := RegQueryStringValue(HKCU, LegacyKey, 'UninstallString', Path);
  if Result then Path := RemoveQuotes(Path);
end;

procedure KillRunningLogiFlowRs();
var
  ResultCode: Integer;
begin
  { 이 앱의 트레이·설정 창만 종료(이미지 이름 LogiFlowRs.exe). }
  Exec(ExpandConstant('{sys}\taskkill.exe'), '/F /IM LogiFlowRs.exe /T', '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
  Sleep(500);
end;

{ 이전 판(C#) 제거 — 트레이를 닫고 그 제거 프로그램을 조용히. 끝날 때까지(제거 정보가 사라질 때까지) 기다린다. }
procedure RemoveLegacyApp();
var
  Path: String;
  ResultCode, I: Integer;
begin
  if not LegacyUninstaller(Path) then Exit;
  Exec(ExpandConstant('{sys}\taskkill.exe'), '/F /IM LogiFlow.exe /T', '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
  Exec(Path, '/VERYSILENT /SUPPRESSMSGBOXES /NORESTART', '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
  for I := 1 to 120 do
  begin
    if not LegacyUninstaller(Path) then Break;
    Sleep(500);
  end;
  LegacyLeft := LegacyUninstaller(Path);
  Log(Format('legacy LogiFlow uninstall rc=%d left=%d', [ResultCode, Ord(LegacyLeft)]));
end;

function InitializeSetup(): Boolean;
var
  Path: String;
begin
  LegacyFound := LegacyUninstaller(Path);
  LegacyLeft := False;
  Result := True;
end;

{ 「설치 준비 완료」 화면에 한 줄 — 이전 판을 바꾼다는 것. }
function UpdateReadyMemo(Space, NewLine, MemoUserInfoInfo, MemoDirInfo, MemoTypeInfo, MemoComponentsInfo, MemoGroupInfo, MemoTasksInfo: String): String;
begin
  Result := MemoDirInfo;
  if MemoTasksInfo <> '' then Result := Result + NewLine + NewLine + MemoTasksInfo;
  if LegacyFound then
    Result := Result + NewLine + NewLine + '기존 LogiFlow(이전 판)를 지우고 이 판으로 바꿉니다. 설정은 처음 실행할 때 이어받습니다.';
end;

function PrepareToInstall(var NeedsRestart: Boolean): String;
begin
  KillRunningLogiFlowRs();
  { 이전 판 바로 가기(같은 이름 「LogiFlow」)를 이전 판 제거 프로그램이 지우므로, 새 바로 가기를 만들기 전에 제거한다. }
  if LegacyFound then RemoveLegacyApp();
  Result := '';
end;

procedure CurStepChanged(CurStep: TSetupStep);
begin
  if (CurStep = ssDone) and LegacyLeft then
    SuppressibleMsgBox('기존 LogiFlow(이전 판, 버전 0.15.x)를 자동으로 지우지 못했습니다.' + #13#10 +
      'Windows 설정 → 앱 → 설치된 앱에서 버전이 0.15.x 인 「LogiFlow」를 찾아 제거해 주세요.', mbInformation, MB_OK, IDOK);
end;

function InitializeUninstall(): Boolean;
begin
  KillRunningLogiFlowRs();
  Result := True;
end;
