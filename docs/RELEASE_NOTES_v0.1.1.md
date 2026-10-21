# LogiFlow v0.1.1 — 진단 열거 확대

- VID 046D HID 인터페이스를 usagePage 필터 없이 전부 로그에 기록(usagePage·usage·caps 읽힘 여부·경로).
- 다른 프로세스가 선점해 못 읽은 기기도 경로에서 VID/PID 를 뽑아 기록(누락 방지).
- HID++ 후보를 벤더 컬렉션(usage 0x0202) 또는 벤더 usagePage(0xFFxx) 20바이트 출력으로 넓힘.
- 여전히 읽기 전용(쓰기/전환 명령 없음).
