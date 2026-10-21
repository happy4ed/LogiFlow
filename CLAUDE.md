# LogiFlow — 로지텍 Easy-Switch 엣지 전환 (회장님 개인 프로젝트)

autographer 마스터 규칙을 상속하되, LogiFlow 는 **자체 origin 을 가진 개인 사이드
프로젝트**다(LocalDex 와 같은 배치). 코드 진본은 이 디렉터리이고 GitHub 비공개 레포
`happy4ed/LogiFlow` 가 remote origin 이다. 마스터 autographer git 추적 대상이 아니다.

## 정체성

- 목표: 로지텍 Easy-Switch 를 모니터 엣지에서 자동 전환(향후). 하드코딩(기능 인덱스
  0x0a 등) 대신 **런타임 조회**로 MX Master 4 를 포함한 어떤 기기든 되게 한다.
- 참조 레포: `happy4ed/Logitech-MX-Auto-Switch`(hidapitester 기반). 우리는 외부 exe
  의존 없이 Windows HID API P/Invoke 로 자체 구현.

## 현재 판: 0.1.0 — 읽기 전용 진단

- 하는 일: HID++ 기기 열거 → IRoot getFeature(0x1814) 인덱스 조회 → 0x1814 func0
  getHostInfo(호스트 개수·현재 호스트) → 기기 이름(0x0005). 직접(0xFF)·리시버(1~6) 모두.
- **안전 불변식: 이번 판은 쓰기(func1 setHost) 절대 안 함. 읽기(func0·0x0000·0x0005)만.
  모르는 명령 전송 금지. 쓰기 경로는 코드에 넣지 않는다.**

## 스택 / 빌드

- C# / .NET Framework 4.6.2, WinForms, Windows 전용. LocalDex 와 같은 구조·CI·패키징.
- 리눅스에서 net462 WinForms 빌드 불가 → CI(windows-latest)로 빌드.
- 릴리스: 커밋 `release:` 접두 또는 workflow_dispatch publish. **push·release 는 main.**
- 커밋 접두 `feat:` (일반 작업).

## 핵심 파일

- `LogiFlow/Native/NativeMethods.cs` — HID/SetupAPI P/Invoke.
- `LogiFlow/Hid/` — 열거(HidEnumerator)·오버랩드 송수신(HidStream).
- `LogiFlow/Hidpp/HidppDevice.cs` — HID++ 프레이밍·IRoot·0x1814·0x0005.
- `LogiFlow/Diagnostics/` — 스캐너(DeviceScanner)·로거(DiagLogger)·결과 모델.
- `LogiFlow/MainForm.cs` — 읽기 전용 진단 창.
