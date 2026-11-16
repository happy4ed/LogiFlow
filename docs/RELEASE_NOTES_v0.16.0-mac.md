# LogiFlow 0.16.0 Mac — Windows 와 같은 설정 창의 새 Mac 앱(러스트 판 첫 Mac 빌드)

Windows 0.16.0 과 같은 코어(HID++ 0x1814 채널 전환 · 가장자리 밀기 · glow · 클립보드 공유)와 **같은 설정 창 · 같은 3화면**(전환 관리 · 설정 · 진단)으로 만든 Mac 앱이다. Swift 판(0.15.0 `LogiFlow.app`)의 창 하나짜리 화면·피커는 쓰지 않는다.

## 받을 파일

| 파일 | 내용 |
|---|---|
| `LogiFlow-mac.dmg` | 설치용 디스크 이미지(Apple Silicon · Intel 한 파일) — 늘 최신을 가리키는 이름 |
| `LogiFlow-v0.16.0-mac.dmg` | 위와 같은 파일(판 번호 붙은 이름) |
| `LogiFlow-v0.16.0-mac.zip` | 같은 앱을 zip 으로 |
| `LogiFlow-v0.16.0-mac-SHA256SUMS.txt` | 해시 |

## 설치와 첫 실행(서명 없는 앱)

Apple 개발자 서명·공증이 없다(ad-hoc 서명만). 그래서 처음 한 번은 macOS 가 막는다.

1. dmg 를 열고 `LogiFlowRs` 를 `Applications` 로 끌어 놓는다. (앱 이름은 LogiFlowRs, 화면에 보이는 이름은 LogiFlow — Swift 판 `LogiFlow.app` 과 나란히 설치된다.)
2. macOS 14 이하: 응용 프로그램 폴더에서 `LogiFlowRs` 를 Control-클릭(오른쪽 클릭) → 「열기」 → 다시 「열기」.
3. macOS 15 이상: 한 번 더블클릭해 「열 수 없음」 안내를 닫고 → 시스템 설정 → 개인정보 보호 및 보안 → 맨 아래 「LogiFlowRs 이(가) 차단되었습니다」 옆 「그래도 열기」 → 암호 → 「그래도 열기」.
4. (터미널을 쓴다면 2·3 대신) `xattr -dr com.apple.quarantine /Applications/LogiFlowRs.app`

한 번 열고 나면 다음부터는 그냥 열린다. 메뉴 막대에 「두 화면 흐름」 아이콘이 생기고 설정 창이 뜬다.

## Swift 판(0.15.0)에서 넘어오기

- 처음 실행할 때 Swift 판 설정 두 가지를 그대로 가져온다: 「변으로 밀어 넘기기」 켜짐/꺼짐(`edgeEnabled`) · 화면 변마다 붙인 채널(`edgeBindings`, 화면 번호 그대로). Swift 판 설정은 지우지 않는다.
- 두 앱을 함께 켜 두면 가장자리 전환이 두 번 일어날 수 있다 — 새 앱이 이전 판이 떠 있는 것을 보면 알림으로 한 번 알린다. Swift 판 메뉴 막대 → 「종료」, 필요하면 Swift 판의 로그인 항목도 끈다.
- 머무는 시간은 Swift 판의 고정 0.5초 대신 Windows 와 같은 설정값(기본 0.33초)을 쓴다 — 설정 창 「머무는 시간」에서 바꿀 수 있다.

## Mac 에서 Windows 와 다른 곳

- 트레이 대신 메뉴 막대 아이콘. 메뉴 항목은 Windows 트레이 메뉴와 같다(현재 채널 · 「… 모두 전환」 · LogiFlow 창 열기 · 기기 다시 찾기 · 종료).
- 「로그인 시 실행」 = `~/Library/LaunchAgents/com.happy4ed.logiflow.rs.plist`(다음 로그인부터). 끄면 파일을 지운다.
- 없는 것(설정 창에서 숨김): 로그인 전에도 실행 · 로그인 화면 테스트 · Windows 방화벽 줄 · 기존 LogiFlow(C# 판) 정리. 클립보드 수신 연결은 macOS 가 처음 한 번 허용할지 묻는다.
- 가장자리 감시는 마우스 이동 이벤트(NSEvent 전역 모니터)로 — 손쉬운 사용·입력 모니터링 권한을 묻지 않는다(Swift 판과 같은 경로).
- 화면 글: 「이 PC」 → 「이 Mac」, 「Windows 시작 시 실행」 → 「로그인 시 실행」 등. 글꼴은 시스템 한글 글꼴(Apple SD Gothic Neo).
- 설정·기기 목록·기록: `~/Library/Application Support/LogiFlowRs/`(`config.txt` · `devices.txt` · `logs/`). 클립보드 암호문구는 `config.txt` 에 평문으로(파일 권한 0600 — 이 사용자만 읽음). Windows 의 DPAPI 같은 보호는 아직 없다.
- 설정 창은 Windows 처럼 열 때만 뜨는 별도 프로세스(닫으면 끝나고, 메뉴 막대 상주는 계속).

클립보드 프레임 · 키 유도 · `config.txt` 형식은 Windows 판(C# 판 포함)과 같다 — Mac ↔ Windows 로 주고받는 형식이 같다.

## CI 로 확인한 것 / 실기(Mac)만 확인할 수 있는 것

CI(macos-latest, 기기 없음)가 확인: 빌드(arm64 · x64 → 유니버설) · clippy · 코어 시험 · 앱 포장과 ad-hoc 서명 검증 · 상주 앱이 뜨고 죽지 않음 · Swift 판 UserDefaults 가짜 값을 이어받아 `config.txt` 에 씀 · IPC 소켓 · 로그인 항목 plist · 두 번째 실행이 설정 창을 엶 · 설정 창 화면 캡처.

실기만(미검증): 로지텍 기기 IOKit 송수신 · 채널 전환 · 가장자리 밀기와 glow 모양 · 메뉴 막대 메뉴 조작 · 로그인 시 자동 시작 · 실제 Swift 판 설정 이어받기(화면 번호가 그대로 맞는지) · Mac ↔ Windows 클립보드 · Gatekeeper 첫 실행 절차.
