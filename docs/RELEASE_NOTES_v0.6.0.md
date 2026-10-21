# LogiFlow 0.6.0

엣지 전환 glow/tension 효과(회장님 10-21 요청). 기능·안전·나머지는 0.5.0 그대로.

## 새 효과

- 커서가 자동 전환 엣지(그 모니터의 그 변)에 닿아 EdgeDetector progress(0→1, dwell/push)가
  쌓이는 동안 그 변에 glow 오버레이:
  - progress 에 따라 색이 흐릿→선명(진해짐). accent 색.
  - 변 쪽으로 당겨지는 tension: 얇은 띠가 progress 로 두꺼워진다(코어 + 가장 밝은 변 선).
    raw push 가 계속 들어오는(커서가 물리 끝에 닿은) 구간을 progress 로 반영.
  - 전환 순간(progress=1) 짧은 pop/flash(흰빛 가미) 후 놓아줌. 엣지를 벗어나면 사라짐(취소).
- 오버레이 = 레이어드·클릭 통과(WS_EX_TRANSPARENT|NOACTIVATE)·최상위·툴윈도우.
  UpdateLayeredWindow per-pixel alpha. 입력 방해 0.
- ~60fps 타이머로 부드럽게 ease. 다중 모니터에서 맞는 모니터의 맞는 변에 배치(4변 지원).
- 자동 전환을 끄거나 배치가 없으면 EdgeWatcher 가 안 돌아 glow 도 뜨지 않음.
- 성능: glow 활성 구간에만 타이머가 돌고(유휴 시 정지), 비트맵은 변 띠 크기만 그린다.

## 구현

- EdgeWatcher 가 progress/state 를 EdgeGlow 로 GlowController 에 전달(UI 스레드). 진단 로그엔
  기존 트리거·샘플만 남긴다(glow 자체는 로그 안 함).

## 제약

- 실기 미검증(빌드 CI windows-latest · 효과는 회장님 실기 필요).
