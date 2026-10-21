# LogiFlow — 로지텍 Easy-Switch 엣지 전환 (회장님 개인 프로젝트)

autographer 마스터 규칙을 상속하되, LogiFlow 는 **자체 origin 을 가진 개인 사이드
프로젝트**다(LocalDex 와 같은 배치). 코드 진본은 이 디렉터리이고 GitHub 비공개 레포
`happy4ed/LogiFlow` 가 remote origin 이다. 마스터 autographer git 추적 대상이 아니다.

## 정체성

- 목표: 로지텍 Easy-Switch 를 모니터 엣지에서 자동 전환(향후). 하드코딩(기능 인덱스
  0x0a 등) 대신 **런타임 조회**로 MX Master 4 를 포함한 어떤 기기든 되게 한다.
- 참조 레포: `happy4ed/Logitech-MX-Auto-Switch`(hidapitester 기반). 우리는 외부 exe
  의존 없이 Windows HID API P/Invoke 로 자체 구현.

## 현재 판: 0.2.0 — 진단 + 전환 테스트 (통제된 쓰기)

- 진단: HID++ 기기 열거(long 0x0202 + short 0x0201 + 벤더 usagePage) → IRoot
  getFeature(0x1814) 인덱스 조회 → 0x1814 func0 getHostInfo → 기기 이름(0x0005).
  직접(0xFF)·리시버(1~6) 전수 핑. short/long 리포트 자동 분기. 중복 기기 합침.
- 목록 정리: 전환 가능(0x1814 지원) 기기만 기본 표시. 빈 슬롯·리시버 자신은 숨김.
  "모두 보기(진단)" 토글(기본 끔)로 전체 열람.
- 전환 테스트: 「채널 N로 전환」 버튼 → 전환 가능한 모든 기기에 각자 setHost(targetIndex)
  전송(마우스·키보드가 같은 PC 로 함께 이동). 기기별 개별 전환 버튼도 있음.
- **안전 불변식(통제된 쓰기): 쓰기는 0x1814 func1 setHost 하나뿐이며, (a)0x1814 지원
  확인된 기기 (b)targetHost 가 0~hostCount-1 범위 (c)명시적 전환 버튼 클릭 — 이 3조건에서만
  호출한다. 그 외 쓰기(SetFeature·SetOutputReport·모르는 func) 금지. setHost 는
  `HostSwitcher` 를 통해서만, TX 바이트는 진단 로그에 남긴다.**
- 설계 메모(다음 판): 전환을 엣지 감지(모니터 캔버스·LocalDex 재사용)로 자동화. 이번 판은 수동 버튼까지.

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
