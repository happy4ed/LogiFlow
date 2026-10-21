# LogiFlow macOS 0.1.0 — HID++ 읽기 진단 (첫 단계)

macOS 판의 첫 단계. 윈도우 버전처럼 **읽기부터 확인**한다. 로지텍 HID++ 기기를 찾아
0x1814 정보(기능 인덱스·호스트 개수·현재 채널)를 읽는 **읽기 전용 진단 도구**다.
전환·엣지·풀 UI 는 다음 단계.

## 들어간 것

- IOHIDManager(IOKit)로 VID 0x046D·usagePage 0xFF43 의 long(0x0202)·short(0x0201)
  벤더 컬렉션 열거.
- 윈도우 HID++ 프로토콜 로직을 그대로 이식 — IRoot getFeature(0x1814) 런타임 인덱스 조회,
  0x1814 func0 호스트 개수·현재 채널, 0x0005 기기 이름, deviceIndex 0xFF·1~6 핑.
- 창: 기기별 이름·VID:PID·연결 방식·deviceIndex·0x1814 인덱스·호스트 개수·현재 채널 ·「새로 고침」.
- 진단 로그: `~/Library/Logs/LogiFlow/logiflow-diag.log` (TX/RX hex·핑 결과).

## 안 들어간 것

- 쓰기(func1 setHost) 없음 — 읽기 전용.
- 호스트 전환·엣지 자동 감지·전체 UI(사이드바·설정·트레이)는 다음 단계.

## 한계

- **실기 미검증** — 리눅스 CI 빌드만 통과. 실제 Mac 에서 MX Master 4 / MX Mechanical 읽히는지
  회장님 확인 필요.
- 미서명·미공증 .app — Gatekeeper 우회 실행 필요(README 참고).
- 다른 앱(Logi Options+)이 기기를 선점하면 조회가 막힐 수 있다(열거는 됨).

## 실행

압축 해제 후 `LogiFlow.app`. 처음 실행 시 미서명이라 Gatekeeper 가 막는다:
우클릭(또는 control-클릭) → 열기 → 확인. 또는 터미널에서
`xattr -dr com.apple.quarantine LogiFlow.app`.
