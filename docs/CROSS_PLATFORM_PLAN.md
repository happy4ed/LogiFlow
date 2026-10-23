# LogiFlow 크로스플랫폼(Mac/Linux) 지원 작업 계획

작성 2026-10-23. **지금 Mac/Linux 런타임 확인 불가** → 이 문서는 "지원 가능하도록" 길을
닦는 계획·설계다. 실제 OS별 런타임 코드는 해당 OS에서 검증 가능해질 때 단계적으로 넣는다
(검증 못 하는 대량 코드 선작성 금지 — 마스터 MVP·과설계 금지 원칙).

## 현 상태

- 윈도우판: C#/.NET Framework 4.6.2 · WinForms · Windows 전용. 빌드는 CI(windows-latest).
- macOS: `mac-v0.1.0` 에 `.dmg`/`.zip` 설치파일만 있음(읽기 전용, 전환·엣지 미검증).
- 전환의 본질(HID++ 2.0 feature 0x1814 Change Host setHost)은 **OS 독립 프로토콜**이다.
  OS마다 다른 건 "그 HID 리포트를 어떤 API로 써 보내느냐"뿐이다.

## 무엇이 공유되고 무엇이 OS별인가

| 계층 | 공유 가능(코어) | OS별 어댑터 |
|------|----------------|-------------|
| HID++ 프로토콜(0x1814·0x0005 프레이밍, setHost/getHostInfo) | ✅ 순수 로직 | — |
| 전환 안전 불변식(쓰기 1곳·3게이트) | ✅ | — |
| 엣지 판정(모니터 경계·dwell·MinPushPx) | ✅ 계산부 | 입력/모니터 데이터 공급은 OS별 |
| HID 장치 열거·리포트 송수신 | ❌ | Win: SetupAPI/HID · mac: IOKit HIDManager · Linux: hidraw(+udev) |
| 커서/raw 입력 감시 | ❌ | Win: RawInput/GetCursorPos · mac: CGEvent tap · Linux: X11/evdev(Wayland 제약) |
| 모니터 열거(경계 좌표) | ❌ | Win: EnumDisplayMonitors · mac: CGGetActiveDisplayList · Linux: XRandR |
| UI(설정·트레이·glow) | ❌ | Win: WinForms · mac: SwiftUI 또는 Avalonia · Linux: Avalonia |
| 부팅 전/로그온 상주 | ❌ | Win: 서비스 · mac: LaunchDaemon · Linux: systemd |

## 목표 아키텍처 — 코어/어댑터 분리

```
LogiFlow.Core        (플랫폼 독립, netstandard2.0)
  ├ Hidpp/           0x1814·0x0005 프레이밍, setHost 로직
  ├ Switching/       HostSwitcher 안전 불변식(IHidTransport 주입받음)
  ├ Edge/            EdgeDetector 계산(입력·모니터 데이터는 인터페이스로 주입)
  └ Abstractions/    IHidTransport · IInputSource · IMonitorSource · IResidentInstaller
LogiFlow.Windows     (net462/net8-windows, 현 코드가 여기로)
LogiFlow.Mac         (net8, IOKit P/Invoke 또는 Swift 브리지)  ← 검증 가능 시
LogiFlow.Linux       (net8, hidraw/libusb · X11/evdev)        ← 검증 가능 시
```

핵심: **지금 윈도우 코드에서 "HID 전송·입력·모니터·상주설치"를 인터페이스 뒤로 밀어내면**,
코어는 그대로 재사용되고 OS별로는 어댑터만 새로 쓴다. 전환 프로토콜(제일 까다로운 부분)은
한 번만 맞으면 전 OS 공유.

## 단계(검증 가능해질 때 순서대로)

1. **코어 추출(윈도우에서 선작업 가능·검증 가능)** — 현 `Hidpp`·`HostSwitcher`·`EdgeDetector`
   에서 Win32 의존을 `IHidTransport`/`IInputSource`/`IMonitorSource` 인터페이스로 분리하고,
   윈도우 어댑터가 그 인터페이스를 구현하게 리팩터. 동작은 지금과 동일(윈도우 CI로 검증).
   → 이 단계는 **지금 해도 안전**(윈도우에서 결과 동일성 확인 가능).
2. **.NET 8 전환 검토** — Framework 4.6.2 는 윈도우 전용. 크로스플랫폼은 .NET 8 필요.
   윈도우판을 net8-windows 로 올리며 WinForms 유지 가능(동작 보존 확인 후).
3. **macOS 어댑터** — IOKit HIDManager 로 Bolt 리시버 0x1814 쓰기 PoC(전환 1회) 먼저.
   되면 CGGetActiveDisplayList(모니터)·CGEvent(입력)·LaunchDaemon(상주) 순.
4. **Linux 어댑터** — hidraw 쓰기 PoC 먼저. X11(XRandR/XInput)은 됨, **Wayland 는 전역 커서/입력
   접근 제약**이 커 한계 고지 필요. systemd 상주.
5. **UI** — Avalonia(맥·리눅스 공용) 또는 각 OS 네이티브. 설정·트레이·수동전환부터, glow 는 후순위.

## 리스크·한계(미리 고지)

- Wayland(최신 리눅스 데스크톱)에서 전역 커서/엣지 감지 제약 → "당기기"만 되고 엣지 전환은 제한될 수 있음.
- 맥은 입력 모니터링·손쉬운 사용 권한(Accessibility/Input Monitoring) 승인 필요.
- 로그온 전 전환(윈도우 서비스처럼)은 OS마다 메커니즘·권한이 전혀 다름 — 각 OS 별건.
- 검증 장비·환경이 없으면 코어 추출(1단계)까지가 "확인하며 할 수 있는 끝", 그 뒤는 실기 필요.

## 지금 이 릴리스(0.13.5)에서 한 것

- 계획 수립(이 문서). 실제 코어 추출/OS 어댑터 코드는 넣지 않음(검증 불가 코드 선작성 회피).
- 다음 안전한 선작업 후보 = **1단계 코어 추출**(윈도우에서 결과 동일성 검증 가능).
