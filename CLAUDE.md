# LogiFlow — 로지텍 Easy-Switch 엣지 전환 (회장님 개인 프로젝트)

autographer 마스터 규칙을 상속하되, LogiFlow 는 **자체 origin 을 가진 개인 사이드
프로젝트**다(LocalDex 와 같은 배치). 코드 진본은 이 디렉터리이고 GitHub 비공개 레포
`happy4ed/LogiFlow` 가 remote origin 이다. 마스터 autographer git 추적 대상이 아니다.

## 정체성

- 목표: 로지텍 Easy-Switch 를 모니터 엣지에서 자동 전환(향후). 하드코딩(기능 인덱스
  0x0a 등) 대신 **런타임 조회**로 MX Master 4 를 포함한 어떤 기기든 되게 한다.
- 참조 레포: `happy4ed/Logitech-MX-Auto-Switch`(hidapitester 기반). 우리는 외부 exe
  의존 없이 Windows HID API P/Invoke 로 자체 구현.

## 현재 판: 0.7.0 — 글자 크기 3단(pt 가산) · 앱 가로 확대 · 캔버스 잘림 제거

- 글자 크기: 작게 −1 · **보통 +1(기본)** · 크게 +3 pt 가산(`UiFont.SetOffset`·`Config.FontOffset`). 비율 스케일 폐지.
- 창 1120×650(최소 980×600). 배치 캔버스 여백 = 모니터 1칸↑(가로 1.3·세로 1.6)로 변 박스·라벨 안 잘림. 미배치 채널 박스는 한 줄에 개수만큼 폭 줄여 전부 표시.
- Mac 설치파일·로그온 전 서비스는 다음 판에서.

## 이전 판: 0.6.0 — 엣지 전환 glow/tension 효과

- 엣지(자동 전환 변)에 커서가 닿아 progress 가 쌓이면 glow 오버레이: 색 진해짐 + 변쪽 띠가
  두꺼워지는 tension + 전환 순간 pop/flash. 레이어드·클릭 통과·최상위(UpdateLayeredWindow,
  per-pixel alpha), ~60fps, 입력 방해 0, 다중 모니터 4변 지원. 자동 전환 끄면 안 뜸.
- `LogiFlow/Ui/GlowOverlay.cs`(GlowOverlayForm+GlowController), `EdgeWatcher` 가 EdgeGlow 전달.

## 이전 판: 0.5.0 — UI 다듬기 (폰트 최소/스케일·테마 스크롤바·가로스크롤 0)

- 폰트: 전역 최소 ≈12.7px 보장 + 글자 크기(보통/크게/아주 크게, %스케일) 설정. 모든 Font 는
  `UiFont.Get`(캐시·최소크기 floor·스케일). dispose 금지.
- 스크롤: 네이티브 대신 `ThemedScrollPanel`+`ThemedScrollBar`(세로 전용 테마 스크롤바).
  콘텐츠 폭=뷰포트 폭 반응형 → 가로 스크롤 없음(스크롤바 자리 항상 예약). 캔버스 320px.
- 기능·안전·캐시·트레이·아이콘·설치파일은 0.4.0 유지.

## 이전 판: 0.4.0 — 승인 UX 시안 구현 (테마·캐시·설치파일)

- 승인 시안(design/ v2) 구현: 사이드바 내비(전환 관리·설정·진단) + 콘텐츠. 커스텀 페인팅
  테마(brief v3 Light/Dark 토큰)·토글·세그먼트·배지·카드. 고정 창(페이지 바꿔도 크기 불변,
  리사이즈·최소크기, 내용 많으면 스크롤).
- 전환 관리: 모니터 배치 캔버스(채널 박스=모니터 크기, 접촉 파랑 자동·분리 점선 수동)·채널
  별칭 편집·기기 목록(전환)·가장자리 자동 전환 옵션.
- 기기 캐시(%APPDATA%\LogiFlow\devices.txt): 시작 시 즉시 표시, 새로 고침·기기 변화
  (WM_DEVICECHANGE 지문 비교) 때만 재스캔.
- 트레이 우클릭: 별칭 포함 바로 전환(현재 제외) + 설정 열기 + 새로 고침 + 종료.
- 앱 아이콘(.ico): 시안 "두 화면 흐름" 마크. 설치파일: Inno Setup per-user(build/windows/LogiFlow.iss).
- **안전 불변식(통제된 쓰기): 쓰기는 0x1814 func1 setHost 하나뿐, `HostSwitcher` 경유로만,
  (a)0x1814 지원 (b)0~hostCount-1 (c)명시 트리거(엣지 or 트레이/버튼). 그 외 쓰기 금지. 엣지는
  설정에서 끌 수 있고 배치 비면 무동작. 로그는 진단 로그 토글.**
- 설계 메모(다음 판): 로그온 전 실행(서비스)·클립보드·ARM 빌드.

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
- `LogiFlow/Config/` — 설정(AppConfig)·자동시작(AutoStart). `LogiFlow/Diagnostics/DeviceCache.cs` — 기기 캐시.
- `LogiFlow/Ui/` — 테마(Theme)·아이콘(Icons)·테마 컨트롤(ThemedControls: 버튼·토글·세그먼트·배지·카드·내비)·앱아이콘(AppIcon).
- `LogiFlow/MonitorLayoutPanel.cs` — 전환 관리 캔버스. `LogiFlow/TrayAppContext.cs` — 트레이·오케스트레이션.
- `LogiFlow/MainForm.cs` — 사이드바 내비 + 3화면 창. `LogiFlow/Resources/logiflow.ico` — 앱 아이콘.
- `build/windows/LogiFlow.iss` — Inno Setup 설치 스크립트. `design/` — 승인 UX 시안.
