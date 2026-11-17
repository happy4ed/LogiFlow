# LogiFlow

## 다운로드 (늘 최신)

| 운영체제 | 받기 |
|---|---|
| Windows | [LogiFlow-windows-setup.exe](https://github.com/happy4ed/LogiFlow/releases/latest/download/LogiFlow-windows-setup.exe) |
| Mac | [LogiFlow-mac.dmg](https://github.com/happy4ed/LogiFlow/releases/latest/download/LogiFlow-mac.dmg) |
| Linux x64 | [LogiFlow-linux-x64.tar.gz](https://github.com/happy4ed/LogiFlow/releases/latest/download/LogiFlow-linux-x64.tar.gz) |
| Linux ARM | [LogiFlow-linux-arm64.tar.gz](https://github.com/happy4ed/LogiFlow/releases/latest/download/LogiFlow-linux-arm64.tar.gz) |

판별 기록은 [Releases](https://github.com/happy4ed/LogiFlow/releases) — 맨 위 Latest 한 판에 세 OS 파일이 같이 있다.

로지텍 Easy-Switch 엣지 전환 프로젝트. 현재 판 0.16.2 — Windows · Mac · Linux 모두 새로 만든 앱(Rust, `rust/`)이
기본이다(가장자리로 밀어 채널 전환 · glow · 클립보드 공유, 더 가볍고 빠름). Linux 는 터미널 CLI 다.
이전 Mac 판(Swift, `mac/`) · Linux 판(`linux/`)은 0.15.0 에서 멈췄다.

### Mac: 처음 열 때

서명하지 않은 앱이라 처음 한 번은 macOS 가 막는다.

- macOS 14 이하: Finder 에서 앱을 Control-클릭 → 「열기」.
- macOS 15 이상: 시스템 설정 → 개인정보 보호 및 보안 → 「그래도 열기」.
- 클립보드 암호 문구는 Mac 키체인에 저장한다. 처음 읽을 때 「항상 허용」을 물을 수 있다.

### Windows: 이전 판(0.15.x)에서 바꾸기

1. [LogiFlow-windows-setup.exe](https://github.com/happy4ed/LogiFlow/releases/latest/download/LogiFlow-windows-setup.exe) 를 실행한다.
   이전 판이 깔려 있으면 설치 파일이 이전 판을 조용히 지우고 새 판을 깐다(관리자 승인 없음).
2. 처음 실행하면 이전 판 설정(별칭 · 배치 · 테마 · 클립보드 · 로그인 자동 시작 등)을 그대로 이어받는다.
   이전 판에서 「로그온 전에도 실행」을 썼다면 한 번 켤지 묻는다(관리자 승인 한 번).
3. 이전 판이 남긴 서비스·파일은 앱의 「기존 LogiFlow 정리」(전환 관리 · 진단 화면)로 지울 수 있다.
4. 클립보드는 이전 판이 깔린 PC 와도 그대로 주고받는다.

아래 1~2절은 초기(0.2.0) 진단 판 설명이 남아 있다(이전 C# 판 기준).

쓰기는 0x1814 func1 setHost 하나뿐이며 (a)0x1814 지원 기기 (b)채널 0~(hostCount-1) 범위
(c)전환 버튼 클릭 — 이 3조건에서만 전송한다. 그 외 쓰기는 하지 않는다.

## 1. 무엇을 하나

1. 로지텍 HID++ 기기 열거 — VID `0x046D`, usagePage `0xFF43` 의 long(`0x0202`) 인터페이스.
2. 각 후보에 대해 런타임 조회:
   - IRoot(feature `0x0000`) getFeature(`0x1814`) 로 Change Host feature 인덱스 조회(하드코딩 없음).
   - `0x1814` func0 getHostInfo 로 호스트 개수·현재 호스트(0-base) 조회.
   - 기기 이름(HID++ `0x0005`, 실패 시 Windows 제품 문자열).
3. 직접 연결(deviceIndex `0xFF`)과 리시버 뒤 페어링(deviceIndex 1~6) 모두 시도.
   short(`0x0201`)·long(`0x0202`) 인터페이스 모두 프로브.
4. 전환 가능(0x1814 지원) 기기만 기본 표시(빈 슬롯·리시버 숨김). "모두 보기(진단)"
   토글로 전체 열람. 기기별 이름 · VID:PID · 연결 방식 · `0x1814` 인덱스 · 호스트 개수 · 현재 채널.
5. 전환 테스트: 「채널 1/2/3로 전환」 버튼으로 전환 가능한 모든 기기를 같은 채널로 함께
   이동(기기별 개별 버튼도 제공). 전환 전 확인 대화상자가 뜬다.

## 2. 실행법

1. 릴리스 ZIP(`LogiFlow-v0.2.0-win.zip`)을 받아 압축을 푼다.
2. `LogiFlow.exe` 실행. .NET Framework 4.6.2 이상이 필요하다(Windows 10/11 기본 포함).
3. 창이 뜨면 자동으로 1회 스캔한다. 다시 읽으려면 「새로 고침」.
4. 전환: 원하는 채널 버튼 클릭 → 확인 → 마우스·키보드가 그 PC 로 넘어간다.
   돌아오려면 그 PC 에서 채널 1 을 누르거나 기기의 Easy-Switch 버튼을 사용한다.

## 3. 읽히면 / 안 읽히면

- **읽히면**: 기기별로 이름·연결 방식·`0x1814` 인덱스·호스트 개수·현재 채널이 보인다.
- **안 읽히면**: 「로그 폴더 열기」 버튼으로 `logs/logiflow-diag.log` 를 연다.
  이 로그에는 보낸 요청/받은 응답 raw 바이트(hex)·타임아웃·오류가 담긴다(serial 등
  민감값 없음). 이 파일을 보내 주면 원인을 분석한다.

안 읽히는 흔한 원인:

- 로지텍 Options+ 등 다른 프로그램이 기기를 점유 중일 수 있다.
- 기기가 꺼져 있거나 다른 채널에 연결되어 있을 수 있다.

## 4. 소스 빌드(개발자용)

- 스택: C# / .NET Framework 4.6.2, WinForms, Windows 전용.
- Windows + Visual Studio(또는 .NET SDK)에서 `LogiFlow.sln` 빌드.
- 리눅스에서는 net462 WinForms 빌드가 불가하므로 CI(windows-latest)로 빌드한다.
- HID 접근은 Windows HID API P/Invoke(SetupDi\* / CreateFile / HidD_\* / ReadFile /
  WriteFile)로 직접 구현했다. 외부 실행 파일(hidapitester 등) 의존은 없다.

## 5. CI / 릴리스

- `.github/workflows/windows-release.yml` 이 windows-latest 에서 빌드하고 ZIP·SHA256 을
  아티팩트로 올린다.
- 릴리스 발행 조건: 커밋 메시지가 `release:` 로 시작하거나, workflow_dispatch 에서
  `publish=true`. (push·release 는 main 담당.)

## 6. 참고

- 전환 기능 사양: HID++ 2.0 feature `0x1814` Change Host.
  - func0 getHostInfo → [호스트 개수, 현재 호스트(0-base)]
  - func1 setHost(index) → 0.2.0 에서 통제된 쓰기로 구현(버튼·범위·지원 검증 조건에서만).
- HID++ 프레임: short 리포트 id `0x10`(7바이트) · long `0x11`(20바이트).
  바이트 구성 `[reportId, deviceIndex, featureIndex, (funcIndex<<4)|softwareId, params...]`.
- 참조 구현: marcocosta97/mxswitch · fducloux/logi-host-switch ·
  antonio-castellon/mx-host-switch · openlogi.org `x1814-change-host` · logiops wiki.
