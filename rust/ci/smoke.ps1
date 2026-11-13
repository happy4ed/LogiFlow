# 기기 없는 CI 러너에서 실행 파일이 뜨고 죽지 않는지(트레이 시작·스캔·종료) — 하드웨어 시험 아님.
param([string]$Exe)
$ErrorActionPreference = 'Stop'
$dir = Split-Path $Exe
$log = Join-Path $dir 'logs\logiflow-diag.log'
if (Test-Path $log) { Remove-Item $log }
$p = Start-Process -FilePath $Exe -PassThru
Start-Sleep -Seconds 12
$alive = -not $p.HasExited
Write-Host "tray alive after 12s: $alive"
if (Test-Path $log) { Get-Content $log -Encoding UTF8 | Select-Object -First 40 | ForEach-Object { Write-Host "  $_" } } else { Write-Host "no log" }
if ($alive) { Stop-Process -Id $p.Id -Force }   # PID 로만 종료
$started = (Test-Path $log) -and (Select-String -Path $log -Pattern 'LogiFlow 시작' -Encoding UTF8 -Quiet)
$scanned = (Test-Path $log) -and (Select-String -Path $log -Pattern '스캔 종료' -Encoding UTF8 -Quiet)
Write-Host "started=$started scanned=$scanned"
# 프로브(읽기 전용) — HID 열거·데스크톱 API 경로가 죽지 않는지.
$probe = Start-Process -FilePath $Exe -ArgumentList '--desktop-probe=5' -PassThru -Wait
Write-Host "probe exit: $($probe.ExitCode)"
$plog = Join-Path $env:ProgramData 'LogiFlowRs\desktop-probe.log'
if (Test-Path $plog) { Get-Content $plog -Encoding UTF8 | Select-Object -First 12 | ForEach-Object { Write-Host "  $_" } }

# 계획서 4.18 — 자동 시작 등록 확인(CI). 설정 autostart=1 로 띄우면 HKCU Run 값 + ONLOGON 예약 작업이 생기고,
# autostart=0 으로 띄우면 둘 다 지워져야 한다(C# AutoStart 와 같은 두 경로). 러너에 남기지 않게 끝은 0.
$cfgDir = Join-Path $env:APPDATA 'LogiFlowRs'
New-Item $cfgDir -ItemType Directory -Force | Out-Null
$cfgPath = Join-Path $cfgDir 'config.txt'
$exeFull = [IO.Path]::GetFullPath($Exe)
function Start-TrayWith([string]$on) {
  Set-Content -Path $cfgPath -Value "autostart=$on" -Encoding utf8
  $t = Start-Process -FilePath $exeFull -PassThru
  Start-Sleep -Seconds 8
  if (-not $t.HasExited) { Stop-Process -Id $t.Id -Force }   # PID 로만 종료
  Start-Sleep -Seconds 1
}
function Get-AutoStart {
  $run = (Get-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run' -Name 'LogiFlowRs' -ErrorAction SilentlyContinue).LogiFlowRs
  & cmd.exe /c 'schtasks /Query /TN LogiFlowRs-Tray >nul 2>&1'   # 없을 때 stderr 가 오류로 번지지 않게 cmd 안에서 버림
  @{ run = $run; task = ($LASTEXITCODE -eq 0) }
}
Start-TrayWith '1'
$a1 = Get-AutoStart
$runOk = $a1.run -and ([IO.Path]::GetFullPath($a1.run.Trim('"')) -ieq $exeFull)
Write-Host "autostart=1 → Run='$($a1.run)' ($runOk) · task=$($a1.task)"
Start-TrayWith '0'
$a0 = Get-AutoStart
Write-Host "autostart=0 → Run='$($a0.run)' · task=$($a0.task)"
$autoOk = $runOk -and $a1.task -and (-not $a0.run) -and (-not $a0.task)
Write-Host "autostart registration ok=$autoOk"
Remove-Item $cfgPath -ErrorAction SilentlyContinue

if (-not ($alive -and $started -and $scanned -and $autoOk)) { exit 1 }
