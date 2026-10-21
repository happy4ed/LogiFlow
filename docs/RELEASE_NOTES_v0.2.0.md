# LogiFlow 0.2.0

목록 정리 + 전환 테스트. 이 판부터 통제된 쓰기(0x1814 func1 setHost) 추가.

## 바뀐 것

- 목록 정리: 전환 가능(0x1814 지원) 기기만 기본 표시. 빈 페어링 슬롯·리시버 자신
  (USB Receiver, HID++1.0 error, 0x1814 없음)은 숨김. "모두 보기(진단)" 토글(기본 끔)로 전체 열람.
- 전환 테스트 UI: 전환 가능 기기와 현재 채널 표시. 「채널 1/2/3로 전환」 버튼 →
  전환 가능한 모든 기기에 각자 setHost(targetIndex) 전송(기기별 feature index·device index
  사용 → 마우스·키보드가 함께 그 PC 로 이동). 기기별 개별 전환 버튼도 제공.
- 쓰기 구현: HidppDevice.SetHost(0x1814 func1, 인자 [targetHostIndex]). long/short 프레이밍
  자동 분기. 전환 즉시 기기가 리셋되므로 fire-and-forget(짧게 drain 후 무시).
- UI 경고 한 줄 + 전환 전 확인 대화상자.

## 안전 가드

- setHost 는 HostSwitcher 를 통해서만, (a)0x1814 지원 확인 기기 (b)채널 0~(hostCount-1) 범위
  (c)버튼 클릭 때만 호출. 그 외 쓰기(SetFeature·SetOutputReport·모르는 func) 없음.
- 전송 TX 바이트는 logiflow-diag.log 에 기록. serial/IP 등 민감값 미기록 유지.

## 실기 기준(10-21, v0.1.2)

- MX Master 4: index2 · 0x1814 0x0E · 3채널 · 현재0.
- MX Mechanical 키보드: index4 · 0x1814 0x09 · 3채널 · 현재0.
- 다른 PC 가 채널 2/3 에 페어링됨(전환 왕복 테스트 가능).

## 제약

- 실기 전환 미검증(빌드 CI windows-latest · 전환 동작은 회장님 실기 필요).
