#!/bin/sh
set -e
if [ "$(id -u)" -ne 0 ]; then
  echo "sudo ./uninstall.sh 로 실행"
  exit 1
fi
rm -f /usr/local/bin/logiflow /etc/udev/rules.d/42-logiflow.rules
udevadm control --reload-rules
echo "삭제 끝"
