#!/bin/sh
# LogiFlow 리눅스 설치: /usr/local/bin/logiflow + udev 규칙(sudo 없이 장치 사용).
set -e
DIR=$(cd "$(dirname "$0")" && pwd)
if [ "$(id -u)" -ne 0 ]; then
  echo "sudo ./install.sh 로 실행"
  exit 1
fi
install -m 0755 "$DIR/logiflow" /usr/local/bin/logiflow
install -m 0644 "$DIR/42-logiflow.rules" /etc/udev/rules.d/42-logiflow.rules
udevadm control --reload-rules
udevadm trigger --subsystem-match=hidraw --action=change
echo "설치 끝 — 예: logiflow list · logiflow switch 2"
