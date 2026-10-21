# LogiFlow 0.10.0

## 로그온 화면 접근 테스트 (실험 · 읽기 전용)

로그인 화면에서 엣지 전환이 실제로 **가능한지**를 회장님 PC(관리형)에서 확인하는 진단 프로브.
진단 화면 맨 아래에 추가. **읽기만 하고 마우스·키보드를 전환하지 않습니다(쓰기 0).**

- [테스트 설치·실행]: 관리자 승인(UAC) 후 SYSTEM 권한 '시작 시(ONSTART)' 예약 작업으로 설치하고 지금 즉시 실행.
  - 설치 자체가 **회사 정책 통과 여부의 1차 판정** — 막히면 그 자리에서 "정책상 불가"가 드러남.
- 프로브 동작(MS 공식 패턴): 매 주기 `OpenInputDesktop`으로 현재 입력 데스크톱(로그인 시 Winlogon)을 열고 이름을 읽은 뒤,
  창·훅 없는 전용 스레드를 `SetThreadDesktop`으로 붙여 `GetCursorPos`로 커서를 읽어 로그로 남김.
- 사용법: 설치 → 바로 Win+L 로 잠금화면에서 마우스 20~30초 움직임 → 돌아와 [로그 열기].
  `desktop="Winlogon"` 줄의 GetCursorPos 가 성공(좌표 변함)이면 **로그인 화면 전환 구현 가능**.
- [제거]로 예약 작업 삭제.

> 조사 근거: 로그인 화면은 보안 데스크톱(Winlogon)이며 LocalSystem 계정만 접근 가능(Microsoft Learn).
> 커서 폴링은 OpenInputDesktop→SetThreadDesktop→GetCursorPos 공식 패턴. Mouse Without Borders 가 System 서비스로
> 잠금/로그인 화면 제어를 실제로 구현. 최대 장벽은 코드 서명·회사 관리 PC 정책·백신 오탐.

## 유지

- 0.9.1 전환 관리 레이아웃(폭 정렬·캔버스 fit·미배치 트레이) 그대로. 쓰기(전환)는 setHost 하나뿐·HostSwitcher 경유.
