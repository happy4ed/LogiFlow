# C# 판(.NET Framework, Windows PowerShell 5.1)과 같은 API 로 클립보드 프레임을 만들어
# 러스트 코어 시험(tests.rs clip_matches_independent_vector)의 기대값과 바이트 대조한다.
# C# ClipboardSync: Trim → NormalizationForm.FormKC → Rfc2898DeriveBytes(20000, SHA1) → GetBytes(32) ×2,
#                   AesManaged CBC PKCS7 + HMACSHA256(IV|암호문).
$ErrorActionPreference = 'Stop'
$utf8 = New-Object System.Text.UTF8Encoding($false)
$raw = $utf8.GetString([Convert]::FromBase64String('ICDruYTrsIDvvJHvvJLvvJMg'))   # "  비밀１２３ "
$text = $utf8.GetString([Convert]::FromBase64String('7JWI64WVIExvZ2lGbG93'))        # "안녕 LogiFlow"
$pw = $raw.Trim().Normalize([Text.NormalizationForm]::FormKC)
$kdf = [System.Security.Cryptography.Rfc2898DeriveBytes]::new($pw, [Text.Encoding]::ASCII.GetBytes("LogiFlow-Clip-v1"), 20000)
$aesKey = $kdf.GetBytes(32); $macKey = $kdf.GetBytes(32)
function Hex($b) { ($b | ForEach-Object { $_.ToString('x2') }) -join '' }
$tb = $utf8.GetBytes($text); $plain = New-Object byte[] ($tb.Length + 1); $plain[0] = 0; [Array]::Copy($tb, 0, $plain, 1, $tb.Length)
$aes = [System.Security.Cryptography.AesManaged]::new()
$aes.Key = $aesKey; $aes.Mode = 'CBC'; $aes.Padding = 'PKCS7'; $aes.IV = [byte[]](0..15)
$ct = $aes.CreateEncryptor().TransformFinalBlock($plain, 0, $plain.Length)
$body = New-Object byte[] (16 + $ct.Length); [Array]::Copy($aes.IV, 0, $body, 0, 16); [Array]::Copy($ct, 0, $body, 16, $ct.Length)
$mac = [System.Security.Cryptography.HMACSHA256]::new($macKey).ComputeHash($body)
$frame = Hex($body) + (Hex $mac)
$expect = @{
  aes = '97a903290fc2f3516137c3579fe3aab2c1b0ea4881ee1d74cf328e9ccfcb0f03'
  mac = '6cff003a638e4677e64008748b82229fd77de5aaf7c7fc4fbe297aa87c0aa317'
  frame = '000102030405060708090a0b0c0d0e0fbc846dc1ac4a60aea7d0599acaa95a646dacf83cc06ac1707d77d2b0ee48baf23e6b4560e6357878290d51a0026fe648b7222aff24bb21628e40689e114b5ae2'
}
$got = @{ aes = (Hex $aesKey); mac = (Hex $macKey); frame = $frame }
$ok = $true
foreach ($k in 'aes','mac','frame') {
  if ($got[$k] -ne $expect[$k]) { Write-Host "MISMATCH $k`n  got    $($got[$k])`n  expect $($expect[$k])"; $ok = $false } else { Write-Host "OK $k" }
}
Write-Host ".NET " ([Environment]::Version) " · PowerShell " $PSVersionTable.PSVersion
if (-not $ok) { exit 1 }
