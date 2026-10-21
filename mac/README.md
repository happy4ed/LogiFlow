# LogiFlow — macOS 판 (HID++ 읽기 진단)

윈도우 버전(`../LogiFlow/`)의 **첫 단계 이식**. 「읽기부터 확인」 — Mac 에서 로지텍 HID++
기기를 찾아 **0x1814 정보(기능 인덱스·호스트 개수·현재 채널)를 읽는 읽기 전용 진단 도구**다.
전환·엣지·풀 UI 는 다음 단계.

윈도우 코드와 분리된 디렉터리(`mac/`)다. 프로토콜 로직은 윈도우 코드를 그대로 옮겼고,
**전송 계층만 IOKit(IOHIDManager)으로 교체**했다.

## 무엇을 하나

1. IOHIDManager 로 VID `0x046D` HID 인터페이스 열거 — usagePage `0xFF43` 의
   long(usage `0x0202`)·short(usage `0x0201`) 벤더 컬렉션.
2. 각 후보에 대해 런타임 조회(하드코딩 없음):
   - IRoot(`0x0000`) getFeature(`0x1814`) 로 Change Host feature 인덱스.
   - `0x1814` func0 getHostInfo 로 호스트 개수·현재 호스트(0-base).
   - 기기 이름(HID++ `0x0005`, 실패 시 제품 문자열).
3. deviceIndex `0xFF`(직접) + 리시버 뒤 `1~6` 모두 핑. 0x1814 미지원 기기도 목록에 표시.
4. 창: 기기별 이름·VID:PID·연결 방식·deviceIndex·`0x1814` 인덱스·호스트 개수·현재 채널 ·「새로 고침」.
5. 진단 로그: `~/Library/Logs/LogiFlow/logiflow-diag.log` (TX/RX hex·핑 결과). serial·IP 는 기록하지 않는다.

**쓰기(func1 setHost)는 하지 않는다.** 읽기(func0·0x0000·0x0005)만.

## 스택 / 빌드

- Swift + SwiftUI, HID 전송 = IOHIDManager(IOKit). macOS 12+.
- Swift Package Manager. 리눅스에선 macOS(AppKit/SwiftUI/IOKit) 빌드 불가 →
  GitHub Actions `macos-latest` 러너(`.github/workflows/mac-release.yml`)로 빌드.
- 로컬 macOS 빌드/패키징:
  ```bash
  cd mac
  ./scripts/make-app.sh          # swift build + .app 조립 + zip/dmg + 체크섬 → mac/dist/
  # 또는 개발 실행:
  swift run
  ```

## 실행 (미서명 .app — Gatekeeper 우회)

코드 서명·공증은 자격 미보유라 생략했다. 처음 실행 시 Gatekeeper 가 막으므로 둘 중 하나:

- **Finder**: `LogiFlow.app` 우클릭(또는 control-클릭) → **열기** → 경고창에서 다시 **열기**.
- **터미널**: `xattr -dr com.apple.quarantine /경로/LogiFlow.app` 후 더블클릭.

창이 뜨면 자동으로 1회 스캔한다. 다시 읽으려면 **새로 고침**(⌘R).

## 핵심 파일

- `Sources/LogiFlowDiag/Hid/HidManager.swift` — IOHIDManager 열거(윈도우 `HidEnumerator` 대응).
- `Sources/LogiFlowDiag/Hid/HidDevice.swift` — SetReport/InputReportCallback 송수신 + 런루프(윈도우 `HidStream`).
- `Sources/LogiFlowDiag/Hidpp/HidppDevice.swift` — HID++ 프레이밍·IRoot·0x1814·0x0005(윈도우 `HidppDevice` 그대로).
- `Sources/LogiFlowDiag/Diagnostics/DeviceScanner.swift` — 스캐너(윈도우 `DeviceScanner` 그대로).
- `Sources/LogiFlowDiag/Diagnostics/DiagLogger.swift` — 진단 로그.
- `Sources/LogiFlowDiag/App/LogiFlowApp.swift` — SwiftUI 창.

## 한계

- **실기 미검증** — 리눅스에서 개발, CI 빌드만 통과 가능. 실제 Mac 에서 읽히는지 확인 필요.
- 미서명·미공증. 다른 앱(Logi Options+)이 기기를 선점하면 조회가 막힐 수 있다(열거는 됨).
