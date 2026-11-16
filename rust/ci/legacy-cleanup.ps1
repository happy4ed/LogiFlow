# 기존 LogiFlow(C# 판) 정리 시험 — 회장님 PC 11-16 사고(앱은 지웠는데 서비스가 남아 「로그인 전에도 실행」을 못 켬) 재현.
# 실제 C# 0.15.0 을 깔고, 「로그온 전에도 실행」이 남기는 것(서비스·예약 작업 3개·%ProgramData%\LogiFlow·도는 워커·
# 방화벽 규칙·Run 값)을 흉내 낸 뒤 `--cleanup-legacy` 를 돌려 전부 사라지고 %APPDATA%\LogiFlow 는 그대로인지 본다.
param([string]$Exe)
$ErrorActionPreference = 'Stop'
# 바깥 명령 실행 → 종료 코드(출력 버림).
function R([string]$exe) { & $exe @args *> $null; return $LASTEXITCODE }

# 1) 실제 C# 판 설치(사용자 범위, 조용히).
$setup = Join-Path $env:RUNNER_TEMP 'LogiFlow-v0.15.0-setup.exe'
gh release download v0.15.0 -p 'LogiFlow-v0.15.0-setup.exe' -D $env:RUNNER_TEMP --clobber
Start-Process $setup -ArgumentList '/VERYSILENT /SUPPRESSMSGBOXES /NORESTART' -Wait
$ukey = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\{B1F3C2A4-7D6E-4C58-9A21-3F0E8C4D5B60}_is1'
$csExe = Join-Path $env:LOCALAPPDATA 'Programs\LogiFlow\LogiFlow.exe'
Write-Host "C# installed: key=$(Test-Path $ukey) exe=$(Test-Path $csExe)"
if (-not (Test-Path $ukey)) { throw 'C# install did not register' }

# 2) 사용자 설정(지우면 안 됨) + 「로그온 전에도 실행」이 남기는 것들.
$appdata = Join-Path $env:APPDATA 'LogiFlow'
New-Item $appdata -ItemType Directory -Force | Out-Null
$sentinel = "edgeEnabled=1`r`nclipSecret=keep-me`r`n"
Set-Content (Join-Path $appdata 'config.txt') $sentinel -NoNewline -Encoding utf8
$md = Join-Path $env:ProgramData 'LogiFlow'
New-Item (Join-Path $md 'worker') -ItemType Directory -Force | Out-Null
Copy-Item "$env:WINDIR\System32\PING.EXE" (Join-Path $md 'worker\LogiFlow.exe')
Set-Content (Join-Path $md 'logon-edges.txt') 'dwellMs=330'
$worker = Start-Process (Join-Path $md 'worker\LogiFlow.exe') -ArgumentList '-n 600 127.0.0.1' -WindowStyle Hidden -PassThru
R sc.exe create LogiFlowLogonSwitch 'binPath=' 'C:\Windows\System32\cmd.exe /c exit' 'start=' demand | Out-Null
R schtasks.exe /Create /TN LogiFlow-PreLogon /TR 'cmd /c exit' /SC ONSTART /RU SYSTEM /F | Out-Null
R schtasks.exe /Create /TN LogiFlow-DesktopProbe /TR 'cmd /c exit' /SC ONSTART /RU SYSTEM /F | Out-Null
R schtasks.exe /Create /TN LogiFlow-Tray /TR 'cmd /c exit' /SC ONLOGON /F | Out-Null
R netsh.exe advfirewall firewall add rule 'name=LogiFlow Clipboard' dir=in action=allow protocol=TCP localport=48737 | Out-Null
$before = @{
  service = (R sc.exe query LogiFlowLogonSwitch) -eq 0
  tasks = @('LogiFlow-PreLogon','LogiFlow-DesktopProbe','LogiFlow-Tray' | Where-Object { (R schtasks.exe /Query /TN $_) -eq 0 }).Count
  dir = Test-Path $md
  fw = (R netsh.exe advfirewall firewall show rule 'name=LogiFlow Clipboard') -eq 0
  run = $null -ne (Get-ItemProperty 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run' -Name LogiFlow -ErrorAction SilentlyContinue)
  worker = -not $worker.HasExited
}
Write-Host ("before: " + (($before.GetEnumerator() | ForEach-Object { "$($_.Key)=$($_.Value)" }) -join ' '))

# 3) 정리(러너는 관리자 — 설정 창에서는 같은 일을 UAC 한 번으로).
$p = Start-Process $Exe -ArgumentList '--cleanup-legacy' -PassThru -Wait
Write-Host "cleanup exit: $($p.ExitCode)"
$plog = Join-Path $env:ProgramData 'LogiFlowRs\desktop-probe.log'
if (Test-Path $plog) { Get-Content $plog -Encoding UTF8 | Select-String '기존 LogiFlow 정리' | ForEach-Object { Write-Host "  $_" } }
$dlog = Join-Path (Split-Path $Exe) 'logs\logiflow-diag.log'
if (Test-Path $dlog) { Get-Content $dlog -Encoding UTF8 | Select-String '기존 LogiFlow 정리' | ForEach-Object { Write-Host "  $_" } }

# 4) 확인.
Start-Sleep -Seconds 1
$after = [ordered]@{
  csKeyGone = -not (Test-Path $ukey)
  csExeGone = -not (Test-Path $csExe)
  serviceGone = (R sc.exe query LogiFlowLogonSwitch) -ne 0
  tasksGone = @('LogiFlow-PreLogon','LogiFlow-DesktopProbe','LogiFlow-Tray' | Where-Object { (R schtasks.exe /Query /TN $_) -eq 0 }).Count -eq 0
  dirGone = -not (Test-Path $md)
  fwGone = (R netsh.exe advfirewall firewall show rule 'name=LogiFlow Clipboard') -ne 0
  runGone = $null -eq (Get-ItemProperty 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run' -Name LogiFlow -ErrorAction SilentlyContinue)
  workerGone = $worker.HasExited
  settingsKept = (Test-Path (Join-Path $appdata 'config.txt')) -and ((Get-Content (Join-Path $appdata 'config.txt') -Raw) -eq $sentinel)
}
$after.GetEnumerator() | ForEach-Object { Write-Host ("  {0,-13} {1}" -f $_.Key, $_.Value) }
if (-not $worker.HasExited) { Stop-Process -Id $worker.Id -Force }   # PID 로만
$bad = $after.GetEnumerator() | Where-Object { -not $_.Value }
if ($bad) { throw "legacy cleanup incomplete: $(($bad | ForEach-Object Key) -join ', ')" }
Write-Host 'legacy cleanup ok'
exit 0
