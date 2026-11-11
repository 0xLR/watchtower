# Local interactive diagnostic. No credentials in arguments, files, or output.
param([switch]$SelfTest)
$ErrorActionPreference = 'Stop'
function Get-Md5([string]$Value) {
    $hash = [Security.Cryptography.MD5]::Create()
    try { return ([BitConverter]::ToString($hash.ComputeHash([Text.Encoding]::UTF8.GetBytes($Value)))).Replace('-', '').ToLowerInvariant() }
    finally { $hash.Dispose() }
}
if ($SelfTest) {
    $ha1 = Get-Md5 'Mufasa:testrealm@host.com:Circle Of Life'
    $ha2 = Get-Md5 'GET:/dir/index.html'
    $response = Get-Md5 ($ha1 + ':dcd98b7102dd2f0e8b11d0f600bfb0c093:00000001:0a4f113b:auth:' + $ha2)
    if ($response -ne '6629fae49393a05397450978507c4ef1') { throw 'Digest reference vector failed' }
    Write-Output 'Digest reference vector passed.'
    exit 0
}
function Read-Headers($Stream) {
    $bytes = New-Object 'System.Collections.Generic.List[byte]'
    while ($bytes.Count -lt 16384) {
        $value = $Stream.ReadByte()
        if ($value -lt 0) { break }
        $bytes.Add([byte]$value)
        if ($bytes.Count -ge 4 -and [Text.Encoding]::ASCII.GetString($bytes.GetRange($bytes.Count - 4, 4).ToArray()) -eq "`r`n`r`n") { break }
    }
    return [Text.Encoding]::UTF8.GetString($bytes.ToArray())
}
function Send-Describe($Stream, [string]$Uri, [int]$Sequence, [string]$Authorization) {
    $header = if ($Authorization) { 'Authorization: ' + $Authorization + "`r`n" } else { '' }
    $request = "DESCRIBE $Uri RTSP/1.0`r`nCSeq: $Sequence`r`nAccept: application/sdp`r`nUser-Agent: go2rtc/1.9.15`r`n$header`r`n"
    $bytes = [Text.Encoding]::UTF8.GetBytes($request)
    $Stream.Write($bytes, 0, $bytes.Length)
    return Read-Headers $Stream
}
try {
    $workspacePath = Split-Path -Parent $PSScriptRoot
    $contextText = & (Join-Path $workspacePath 'target/debug/examples/rtsp_status.exe') --connection-context Wi-Fi 2>$null
    if ($LASTEXITCODE -ne 0) { throw 'Camera discovery failed' }
    $context = $contextText | ConvertFrom-Json
    $cameraCredential = $Host.UI.PromptForCredential(
        'Watchtower Digest authentication diagnostic',
        'Enter the C530WS local RTSP camera account to test Digest authentication. Credentials stay in memory; only the status is returned.', '', '')
    if ($null -eq $cameraCredential) { Write-Output 'Camera diagnostic cancelled.'; exit 0 }
    $cameraAccount = $cameraCredential.GetNetworkCredential()
    $socket = New-Object Net.Sockets.TcpClient
    $connect = $socket.ConnectAsync($context.host, [int]$context.port)
    if (-not $connect.Wait(5000)) { throw 'Connection timed out' }
    $stream = $socket.GetStream()
    $stream.ReadTimeout = 5000
    $stream.WriteTimeout = 5000
    $uri = 'rtsp://' + $context.host + ':' + $context.port + '/stream1'
    $challenge = Send-Describe $stream $uri 1 ''
    $digest = ($challenge -split "`r`n" | Where-Object { $_ -match '^WWW-Authenticate:\s*Digest ' } | Select-Object -First 1)
    if (-not $digest) { throw 'Camera did not offer Digest' }
    $realm = [regex]::Match($digest, 'realm="([^"]*)"').Groups[1].Value
    $nonce = [regex]::Match($digest, 'nonce="([^"]*)"').Groups[1].Value
    $qop = [regex]::Match($digest, 'qop="([^"]*)"').Groups[1].Value
    $ha1 = Get-Md5 ($cameraAccount.UserName + ':' + $realm + ':' + $cameraAccount.Password)
    $ha2 = Get-Md5 ('DESCRIBE:' + $uri)
    $cnonce = [Guid]::NewGuid().ToString('N')
    if ($qop) {
        if ('auth' -notin ($qop -split ',').Trim()) { throw 'Unsupported Digest qop' }
        $response = Get-Md5 ($ha1 + ':' + $nonce + ':00000001:' + $cnonce + ':auth:' + $ha2)
        $extra = ', qop=auth, nc=00000001, cnonce="' + $cnonce + '"'
    } else {
        $response = Get-Md5 ($ha1 + ':' + $nonce + ':' + $ha2)
        $extra = ''
    }
    $authorization = 'Digest username="' + $cameraAccount.UserName + '", realm="' + $realm + '", nonce="' + $nonce + '", uri="' + $uri + '", response="' + $response + '"' + $extra
    $result = Send-Describe $stream $uri 2 $authorization
    $code = [regex]::Match($result, '^RTSP/1\.0 ([1-5][0-9][0-9])').Groups[1].Value
    if (-not $code) { throw 'Invalid RTSP status' }
    Write-Output ('model=C530WS path=/stream1 authentication=Digest status=' + $code + ' qop=' + [bool]$qop)
} catch {
    # Do not emit exception messages: socket/parser errors can contain input.
    Write-Output 'Digest diagnostic could not complete. No credentials were saved.'
    exit 1
} finally {
    if ($socket) { $socket.Dispose() }
    $cameraAccount = $null
    $cameraCredential = $null
    $authorization = $null
    $ha1 = $null
}
