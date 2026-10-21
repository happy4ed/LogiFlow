# LogiFlow 0.1.0

로지텍 Easy-Switch 엣지 전환 프로젝트의 첫 빌드. 읽기 전용 진단 도구.

## 담긴 것

- 로지텍 HID++ 기기 열거(VID 0x046D, usagePage 0xFF43, long 0x0202).
- 런타임 feature 인덱스 조회: IRoot getFeature(0x1814) — 하드코딩 없음.
- 0x1814 func0 getHostInfo: 호스트 개수·현재 호스트(0-base).
- 기기 이름: HID++ 0x0005, 실패 시 Windows 제품 문자열.
- 직접 연결(deviceIndex 0xFF)·리시버 뒤(1~6) 모두 탐지.
- 간단한 WinForms 창: 기기별 이름·VID:PID·경로·연결 방식·0x1814 인덱스·호스트 개수·현재 채널 + 「새로 고침」.
- 진단 로그: logs/logiflow-diag.log (요청/응답 raw hex·타임아웃·오류).

## 안 담긴 것(의도적)

- 호스트 전환(func1 setHost) 등 쓰기 명령 — 코드에 없음. 읽기 전용.
- 트레이·엣지·전환 버튼 — 다음 판 이후.

## 제약

- 실기 HID 미검증(빌드는 CI windows-latest, 동작 확인은 회장님 MX Master 4 실기 필요).
