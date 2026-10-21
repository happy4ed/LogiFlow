# LogiFlow 0.3.0

핵심 기능: 엣지에서 자동 전환. 트레이 상주 + 자동 시작.

## 새 기능

- PC 위치 설정 캔버스: 이 PC 의 모니터(Screen.AllScreens)를 비율로 그리고, 다른 채널 PC
  박스(「채널 2 PC」·「채널 3 PC」)를 끌어다 모니터 변(왼/오/위/아래)에 붙인다. 붙인 변 =
  그 채널로 넘어가는 엣지. 미배치 채널은 엣지 전환 안 함. 현재 채널은 캔버스 머리에 표시.
- 엣지 자동 전환: 저수준 raw 마우스로 커서가 설정된 변(그 모니터, 2px 띠)에 dwell(기본
  0.33초) + 바깥 밀기 하면 HostSwitcher.SwitchAll(그 변 채널) 호출(마우스·키보드 함께 이동).
  드래그 중(버튼 누름)·주입 입력은 트리거 안 함, 핑퐁 방지 재무장.
- 트레이 상주: 트레이 아이콘(현재 채널·엣지 상태 표시, 설정 열기·새로 고침·종료). 창을
  닫으면 트레이로 내려감. 로그인 시 자동 시작 기본 켬(HKCU Run).
- 설정: 캔버스 배치 + 엣지 dwell + 엣지 사용 토글 + 자동시작 토글. %APPDATA%\LogiFlow\config.txt 저장.

## LocalDex 재사용

- 엣지 판정 로직(InputHandoffEdgeDetector)·레이아웃 스냅(ResolveAttachment/SnapFlush)·
  raw input 처리(RegisterRawInputDevices·WM_INPUT·TryReadRelativeMouse)·논리좌표 방식
  (GetCursorPos + Screen.Bounds)을 이식·적응. 출처 주석. 라이선스 양쪽 회장님 소유.

## 안전

- 쓰기는 0x1814 func1 setHost 하나뿐, HostSwitcher 경유만: (a)0x1814 지원 (b)0~hostCount-1
  범위 (c)명시 트리거(엣지 or 버튼). 그 외 쓰기(SetFeature·SetOutputReport·모르는 func) 없음.
- 엣지 전환은 설정에서 끌 수 있고, 배치가 비면 아무 동작 안 함.
- 엣지 판정·전환 트리거는 logiflow-diag.log 에 기록.
- 전환 후엔 이 PC 에서 기기가 사라져 이 PC 는 더 명령을 못 보냄 — 복귀는 대상 PC 의
  LogiFlow 나 기기 Easy-Switch 버튼.

## 다음 판(이번 아님)

- 로그온 전 동작·설치파일·클립보드·ARM 빌드.

## 제약

- 실기 엣지/전환 미검증(빌드 CI windows-latest · 동작은 회장님 실기 필요).
