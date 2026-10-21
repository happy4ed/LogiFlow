# LogiFlow — 로지텍 Easy-Switch 엣지 전환 (회장님 개인 프로젝트)

autographer 마스터 규칙을 상속하되, LogiFlow 는 **자체 origin 을 가진 개인 사이드
프로젝트**다(LocalDex 와 같은 배치). 코드 진본은 이 디렉터리이고 GitHub 비공개 레포
`happy4ed/LogiFlow` 가 remote origin 이다. 마스터 autographer git 추적 대상이 아니다.

## 정체성

- 목표: 로지텍 Easy-Switch 를 모니터 엣지에서 자동 전환(향후). 하드코딩(기능 인덱스
  0x0a 등) 대신 **런타임 조회**로 MX Master 4 를 포함한 어떤 기기든 되게 한다.
- 참조 레포: `happy4ed/Logitech-MX-Auto-Switch`(hidapitester 기반). 우리는 외부 exe
  의존 없이 Windows HID API P/Invoke 로 자체 구현.

## 현재 판: 0.3.0 — 엣지 자동 전환 (트레이 상주)

- 진단·수동 전환(0.1.x~0.2.0 상속): HID++ 열거·0x1814 런타임 조회·getHostInfo·
  수동 전환 버튼. 전환 가능 기기만 기본 표시, "모두 보기(진단)" 토글.
- 엣지 자동 전환: 설정 캔버스에서 다른 채널 PC 박스를 모니터 변에 배치 → 커서가 그 변의
  2px 띠에 dwell(기본 0.33초)+바깥밀기 하면 그 채널로 전환 가능한 모든 기기를 함께 이동.
  raw 마우스(WM_INPUT)·논리좌표(GetCursorPos+Screen.Bounds)·엣지 판정은 LocalDex 이식.
- 트레이 상주 + 로그인 자동 시작(HKCU Run). 창 닫으면 트레이로. 설정은 %APPDATA%\LogiFlow\config.txt.
- **안전 불변식(통제된 쓰기): 쓰기는 0x1814 func1 setHost 하나뿐이며, `HostSwitcher` 경유로만,
  (a)0x1814 지원 (b)targetHost 0~hostCount-1 범위 (c)명시 트리거(엣지 or 버튼) — 이 3조건에서만.
  그 외 쓰기(SetFeature·SetOutputReport·모르는 func) 금지. 엣지는 설정에서 끌 수 있고 배치가
  비면 무동작. 엣지 판정·전환 트리거는 진단 로그에 남긴다.**
- 설계 메모(다음 판): 로그온 전 동작·설치파일·클립보드·ARM 빌드.

## 스택 / 빌드

- C# / .NET Framework 4.6.2, WinForms, Windows 전용. LocalDex 와 같은 구조·CI·패키징.
- 리눅스에서 net462 WinForms 빌드 불가 → CI(windows-latest)로 빌드.
- 릴리스: 커밋 `release:` 접두 또는 workflow_dispatch publish. **push·release 는 main.**
- 커밋 접두 `feat:` (일반 작업).

## 핵심 파일

- `LogiFlow/Native/NativeMethods.cs` — HID/SetupAPI P/Invoke.
- `LogiFlow/Hid/` — 열거(HidEnumerator)·오버랩드 송수신(HidStream).
- `LogiFlow/Hidpp/HidppDevice.cs` — HID++ 프레이밍·IRoot·0x1814·0x0005.
- `LogiFlow/Diagnostics/` — 스캐너(DeviceScanner)·로거(DiagLogger)·결과 모델·전환(HostSwitcher).
- `LogiFlow/Handoff/` — 엣지 판정(EdgeDetector·LocalDex 이식)·raw 마우스 감시(EdgeWatcher).
- `LogiFlow/Config/` — 설정(AppConfig)·자동시작(AutoStart).
- `LogiFlow/MonitorLayoutPanel.cs` — 모니터 배치 캔버스. `LogiFlow/TrayAppContext.cs` — 트레이 상주·오케스트레이션.
- `LogiFlow/MainForm.cs` — 설정 창(진단·전환 탭 + 엣지 자동전환 탭).
