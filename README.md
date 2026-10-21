# LogiFlow

로지텍 Easy-Switch 엣지 전환 프로젝트의 첫 빌드. 이번 판은 **읽기 전용 진단 도구**다.
전환·엣지 기능은 아직 없다. MX Master 4 등 로지텍 기기에서 HID++ 정보를 제대로
읽어오는지만 확인한다.

## 1. 무엇을 하나

1. 로지텍 HID++ 기기 열거 — VID `0x046D`, usagePage `0xFF43` 의 long(`0x0202`) 인터페이스.
2. 각 후보에 대해 런타임 조회:
   - IRoot(feature `0x0000`) getFeature(`0x1814`) 로 Change Host feature 인덱스 조회(하드코딩 없음).
   - `0x1814` func0 getHostInfo 로 호스트 개수·현재 호스트(0-base) 조회.
   - 기기 이름(HID++ `0x0005`, 실패 시 Windows 제품 문자열).
3. 직접 연결(deviceIndex `0xFF`)과 리시버 뒤 페어링(deviceIndex 1~6) 모두 시도.
4. 결과를 간단한 창에 표시: 이름 · VID:PID · 경로 · 연결 방식 · `0x1814` 인덱스 ·
   호스트 개수 · 현재 채널.

이번 판은 **읽기만** 한다. 호스트 전환(func1 setHost) 등 쓰기 명령은 전송하지 않으며
코드에도 없다.

## 2. 실행법

1. 릴리스 ZIP(`LogiFlow-v0.1.0-win.zip`)을 받아 압축을 푼다.
2. `LogiFlow.exe` 실행. .NET Framework 4.6.2 이상이 필요하다(Windows 10/11 기본 포함).
3. 창이 뜨면 자동으로 1회 스캔한다. 다시 읽으려면 「새로 고침」.

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
  - func1 setHost(index) → 이번 판에서는 구현하지 않음(읽기 전용).
- HID++ 프레임: short 리포트 id `0x10`(7바이트) · long `0x11`(20바이트).
  바이트 구성 `[reportId, deviceIndex, featureIndex, (funcIndex<<4)|softwareId, params...]`.
- 참조 구현: marcocosta97/mxswitch · fducloux/logi-host-switch ·
  antonio-castellon/mx-host-switch · openlogi.org `x1814-change-host` · logiops wiki.
