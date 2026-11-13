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
if (-not ($alive -and $started -and $scanned)) { exit 1 }
