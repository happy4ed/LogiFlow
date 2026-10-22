# LogiFlow 0.10.4

## 로그온 화면 프로브 — SetThreadDesktop err170 수정

2차(0.10.2)에서 세션 1 winlogon 진입·입력 데스크톱 이름 읽기는 됐으나 `SetThreadDesktop err170(ERROR_BUSY)` 로 커서를 못 읽었다.

원인: `[STAThread]` 메인 스레드는 .NET 이 STA COM 용 **숨은 창**을 만들어, "창·훅 있는 스레드는 SetThreadDesktop 불가"(MS 문서) 규칙에 걸림.

수정: 데스크톱 전환·커서 읽기를 **창·메시지큐 없는 새 MTA 워커 스레드**에서 수행. 이제 `desktop="Winlogon"` 에서 `SetThreadDesktop 성공` + `GetCursorPos (좌표)` 가 나와야 한다.

- 진단 → [테스트 설치·실행] → Win+L → 마우스 움직임 → [로그 열기]. `desktop="Winlogon" setThreadDesktop=성공 GetCursorPos=(x,y)` 이고 좌표가 마우스 따라 바뀌면 → **로그인 화면에서 엣지 감지·전환 구현 가능**.
- 읽기 전용 유지(HID/setHost 없음).

## 유지

- 0.10.3 업데이트 시 이전 실행본 자동 종료 그대로.
