# LogiFlow 0.10.2

## 로그온 화면 접근 테스트 — 2차(세션 1 winlogon 진입)

1차(0.10.0)에서 SYSTEM 예약 작업이 **세션 0**(서비스 세션)으로 돌아 입력 데스크톱이 없어 `OpenInputDesktop err=1` 로 실패했다(세션 0 격리 · 예상된 결과).

2차: 세션 0 런처가 **콘솔 세션의 winlogon.exe 토큰을 복제**해 `winsta0\winlogon` 데스크톱으로 프로브를 다시 띄운다(Mouse Without Borders 방식). 그 프로브가 로그인/잠금 화면에서 `OpenInputDesktop`+`SetThreadDesktop`+`GetCursorPos` 로 커서를 읽을 수 있는지 로그로 남긴다.

- 진단 → [테스트 설치·실행] → 바로 **Win+L** 로 잠금화면에서 마우스 20~30초 → 돌아와 [로그 열기].
- `CreateProcessAsUser(winsta0\winlogon) 성공` + 이어지는 프로브에서 `desktop="Winlogon"` 의 `GetCursorPos 성공(좌표 변함)` 이면 로그인 화면 전환 구현 가능.
- 여전히 읽기 전용(HID/setHost 없음).

## 유지

- 0.10.1 전환 관리(박스 분실 방지·칩 균등 폭) 그대로.
