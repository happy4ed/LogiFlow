; LogiFlow 러스트 시험판 — 사용자 범위 설치(Inno Setup). C# 판(build/windows/LogiFlow.iss)과 나란히 설치된다:
; AppId·폴더·실행 파일 이름이 모두 다르다. x64·arm64 실행 파일을 한 설치 파일에 담아 PC 에 맞는 쪽을 깐다.
; 시험판 한정: 로그인 자동 시작(HKCU Run)을 설치 때 등록하지 않는다(C# 판과 두 트레이 동시 자동 시작 방지).
; 제거 때 서비스·워커 사본·예약 작업·방화벽 규칙까지 정리한다(--uninstall-cleanup, 필요할 때만 UAC).
#ifndef AppVersion
  #define AppVersion "0.16.0-rs.3"
#endif
#ifndef FileVersion
  #define FileVersion "0.16.0.3"
#endif
#ifndef DistDir
  #define DistDir "..\dist"
#endif
#ifndef OutputDir
  #define OutputDir "..\dist"
#endif

[Setup]
AppId={{6F3B2D1A-9C84-4E57-B0A3-7D52E1C9F046}
AppName=LogiFlow (러스트 시험판)
AppVersion={#AppVersion}
AppVerName=LogiFlow (러스트 시험판) {#AppVersion}
VersionInfoVersion={#FileVersion}
AppPublisher=happy4ed
AppPublisherURL=https://github.com/happy4ed/LogiFlow
AppSupportURL=https://github.com/happy4ed/LogiFlow/issues
DefaultDirName={localappdata}\Programs\LogiFlowRs
DefaultGroupName=LogiFlow (러스트 시험판)
PrivilegesRequired=lowest
MinVersion=10.0
ArchitecturesAllowed=x64compatible arm64
ArchitecturesInstallIn64BitMode=x64compatible arm64
OutputDir={#OutputDir}
OutputBaseFilename=LogiFlowRs-v{#AppVersion}-setup
SetupIconFile=..\..\LogiFlow\Resources\logiflow.ico
UninstallDisplayIcon={app}\LogiFlowRs.exe
UninstallDisplayName=LogiFlow (러스트 시험판)
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
Name: "{group}\LogiFlow (러스트 시험판)"; Filename: "{app}\LogiFlowRs.exe"; WorkingDir: "{app}"
Name: "{autodesktop}\LogiFlow (러스트 시험판)"; Filename: "{app}\LogiFlowRs.exe"; WorkingDir: "{app}"; Tasks: desktopicon

[Run]
Filename: "{app}\LogiFlowRs.exe"; Description: "{cm:LaunchProgram,LogiFlow (러스트 시험판)}"; Flags: nowait postinstall skipifsilent

[UninstallRun]
; 파일을 지우기 전에 실행된다. 머신 범위에 남은 것이 있으면 그때만 UAC 를 묻는다.
Filename: "{app}\LogiFlowRs.exe"; Parameters: "--uninstall-cleanup"; Flags: waituntilterminated; RunOnceId: "LogiFlowRsCleanup"

[UninstallDelete]
Type: filesandordirs; Name: "{app}\logs"

[Code]
procedure KillRunningLogiFlowRs();
var
  ResultCode: Integer;
begin
  { 실행 중인 러스트 시험판 트레이·설정 창만 종료(이미지 이름 LogiFlowRs.exe — C# 판 LogiFlow.exe 는 건드리지 않음). }
  Exec(ExpandConstant('{sys}\taskkill.exe'), '/F /IM LogiFlowRs.exe /T', '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
  Sleep(500);
end;

function PrepareToInstall(var NeedsRestart: Boolean): String;
begin
  KillRunningLogiFlowRs();
  Result := '';
end;

function InitializeUninstall(): Boolean;
begin
  KillRunningLogiFlowRs();
  Result := True;
end;
