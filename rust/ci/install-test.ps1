# 설치·업그레이드·제거 경로 시험(0.16.0~ — 이 앱이 Windows 기본 판). ⑥ 0.16.1~: 공개 0.16.0 위 제자리 업그레이드.
# ① 실제 C# 0.15.0 설치 + 그 설정(자동 시작·로그온 전에도 실행 켬) → ② 새 설치 파일 실행: 이전 판이 지워지고 이 앱이 깔리는지,
# ③ 처음 실행: 설정을 값 그대로 이어받는지(로그온 전에도 실행만 꺼 두고 묻기), ④ 같은 설치 파일로 다시 설치(제자리 업그레이드),
# ⑤ 제거: 앱·자동 시작이 지워지고 사용자 설정은 남는지.
param([string]$Setup, [string]$Version)
$ErrorActionPreference = 'Stop'
function Invoke-Native([string]$exe) { & $exe @args *> $null; return $LASTEXITCODE }
$legacyKey = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\{B1F3C2A4-7D6E-4C58-9A21-3F0E8C4D5B60}_is1'
$ourKey = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\{6F3B2D1A-9C84-4E57-B0A3-7D52E1C9F046}_is1'
$runKey = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run'
$csExe = Join-Path $env:LOCALAPPDATA 'Programs\LogiFlow\LogiFlow.exe'
$ourExe = Join-Path $env:LOCALAPPDATA 'Programs\LogiFlowRs\LogiFlowRs.exe'
$lnk = Join-Path $env:APPDATA 'Microsoft\Windows\Start Menu\Programs\LogiFlow\LogiFlow.lnk'
$checks = [ordered]@{}
function Check([string]$name, [bool]$ok) { $checks[$name] = $ok; Write-Host ("  {0,-34} {1}" -f $name, $ok) }

# 앞 단계(실행 시험)가 만든 이 앱 설정은 지워 「처음 설치」로.
Remove-Item (Join-Path $env:APPDATA 'LogiFlowRs') -Recurse -Force -ErrorAction SilentlyContinue

# ① 이전 판 + 그 설정.
$old = Join-Path $env:RUNNER_TEMP 'LogiFlow-v0.15.0-setup.exe'
if (-not (Test-Path $old)) { gh release download v0.15.0 -p 'LogiFlow-v0.15.0-setup.exe' -D $env:RUNNER_TEMP --clobber }
Start-Process $old -ArgumentList '/VERYSILENT /SUPPRESSMSGBOXES /NORESTART' -Wait
$csCfgDir = Join-Path $env:APPDATA 'LogiFlow'
New-Item $csCfgDir -ItemType Directory -Force | Out-Null
$csCfg = "edgeEnabled=1`r`ndwellMs=500`r`nautostart=1`r`npreLogon=1`r`ntheme=2`r`nalias=1`t노트북`r`n"
Set-Content (Join-Path $csCfgDir 'config.txt') $csCfg -NoNewline -Encoding utf8
Check 'legacy installed' (Test-Path $legacyKey)

# ② 새 설치 파일(조용히).
$a1 = "/VERYSILENT /SUPPRESSMSGBOXES /NORESTART /LOG=`"$env:RUNNER_TEMP\setup1.log`""
Start-Process $Setup -ArgumentList $a1 -Wait
Get-Content "$env:RUNNER_TEMP\setup1.log" -ErrorAction SilentlyContinue | Select-String 'legacy LogiFlow' | ForEach-Object { Write-Host "  setup: $_" }
Check 'legacy uninstall key gone' (-not (Test-Path $legacyKey))
Check 'legacy exe gone' (-not (Test-Path $csExe))
Check 'our exe installed' (Test-Path $ourExe)
$k = Get-ItemProperty $ourKey -ErrorAction SilentlyContinue
Check 'Apps entry name = LogiFlow' ($k -and $k.DisplayName -eq 'LogiFlow')
Check "Apps entry version = $Version" ($k -and $k.DisplayVersion -eq $Version)
$target = if (Test-Path $lnk) { (New-Object -ComObject WScript.Shell).CreateShortcut($lnk).TargetPath } else { '' }
Check 'Start menu LogiFlow -> our exe' ($target -ieq $ourExe)
Check 'legacy settings kept' (Test-Path (Join-Path $csCfgDir 'config.txt'))

# ③ 처음 실행 — 설정 이어받기.
$t = Start-Process $ourExe -PassThru
Start-Sleep -Seconds 10
if (-not $t.HasExited) { Stop-Process -Id $t.Id -Force }   # PID 로만
Start-Sleep -Seconds 1
$our = Join-Path $env:APPDATA 'LogiFlowRs\config.txt'
$txt = if (Test-Path $our) { Get-Content $our -Raw -Encoding UTF8 } else { '' }
Check 'imported autostart=1' ($txt -match '(?m)^autostart=1')
Check 'imported dwellMs=500' ($txt -match '(?m)^dwellMs=500')
Check 'imported theme=2' ($txt -match '(?m)^theme=2')
Check 'preLogon left off (asked once)' ($txt -match '(?m)^preLogon=0')
$diag = Join-Path (Split-Path $ourExe) 'logs\logiflow-diag.log'
$imp = if (Test-Path $diag) { (Get-Content $diag -Encoding UTF8 | Select-String '설정 이어받기' | Select-Object -First 1).Line } else { '' }
Write-Host "  log: $imp"
Check 'import log says preLogon=1 to ask' ($imp -match 'preLogon=1')
$run = (Get-ItemProperty $runKey -Name LogiFlowRs -ErrorAction SilentlyContinue).LogiFlowRs
Check 'autostart registered (Run LogiFlowRs)' ($run -and $run.Trim('"') -ieq $ourExe)

# ④ 같은 설치 파일로 다시(제자리 업그레이드) — 항목 하나, 설정 그대로.
Start-Process $Setup -ArgumentList '/VERYSILENT /SUPPRESSMSGBOXES /NORESTART' -Wait
Check 'upgrade: exe still there' (Test-Path $ourExe)
Check 'upgrade: settings unchanged' ((Get-Content $our -Raw -Encoding UTF8) -eq $txt)

# ⑤ 제거.
$unins = Join-Path (Split-Path $ourExe) 'unins000.exe'
Start-Process $unins -ArgumentList '/VERYSILENT /SUPPRESSMSGBOXES /NORESTART' -Wait
for ($i = 0; $i -lt 60 -and (Test-Path $ourKey); $i++) { Start-Sleep -Milliseconds 500 }
Check 'uninstall: Apps entry gone' (-not (Test-Path $ourKey))
Check 'uninstall: exe gone' (-not (Test-Path $ourExe))
Check 'uninstall: Run value gone' ($null -eq (Get-ItemProperty $runKey -Name LogiFlowRs -ErrorAction SilentlyContinue))
Check 'uninstall: tray task gone' ((Invoke-Native schtasks.exe /Query /TN LogiFlowRs-Tray) -ne 0)
Check 'uninstall: user settings kept' (Test-Path $our)

# ⑥ 0.16.1~: 공개된 0.16.0(Windows 기본 판) 위 제자리 업그레이드 — 같은 AppId 라 Apps 항목 하나 · 판 번호만 바뀌고 설정 그대로.
if ($Version -ne '0.16.0') {
  $prev = Join-Path $env:RUNNER_TEMP 'LogiFlow-v0.16.0-setup.exe'
  if (-not (Test-Path $prev)) { gh release download v0.16.0 -p 'LogiFlow-v0.16.0-setup.exe' -D $env:RUNNER_TEMP --clobber }
  Start-Process $prev -ArgumentList '/VERYSILENT /SUPPRESSMSGBOXES /NORESTART' -Wait
  $k0 = Get-ItemProperty $ourKey -ErrorAction SilentlyContinue
  Check 'prev 0.16.0 installed' ($k0 -and $k0.DisplayVersion -eq '0.16.0')
  $before = Get-Content $our -Raw -Encoding UTF8
  Start-Process $Setup -ArgumentList '/VERYSILENT /SUPPRESSMSGBOXES /NORESTART' -Wait
  $k1 = Get-ItemProperty $ourKey -ErrorAction SilentlyContinue
  Check "0.16.0 -> $Version version" ($k1 -and $k1.DisplayVersion -eq $Version)
  $entries = @(Get-ChildItem 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall' | ForEach-Object { Get-ItemProperty $_.PSPath } | Where-Object { $_.DisplayName -eq 'LogiFlow' })
  Check '0.16.0 -> new: one Apps entry' ($entries.Count -eq 1)
  Check '0.16.0 -> new: exe there' (Test-Path $ourExe)
  Check '0.16.0 -> new: settings unchanged' ((Get-Content $our -Raw -Encoding UTF8) -eq $before)
  Start-Process (Join-Path (Split-Path $ourExe) 'unins000.exe') -ArgumentList '/VERYSILENT /SUPPRESSMSGBOXES /NORESTART' -Wait
  for ($i = 0; $i -lt 60 -and (Test-Path $ourKey); $i++) { Start-Sleep -Milliseconds 500 }
  Check '0.16.0 -> new: uninstall clean' (-not (Test-Path $ourKey))
}

$bad = $checks.GetEnumerator() | Where-Object { -not $_.Value }
if ($bad) { throw "install paths failed: $(($bad | ForEach-Object Key) -join ', ')" }
Write-Host 'install / upgrade / uninstall paths ok'
exit 0
