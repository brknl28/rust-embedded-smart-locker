param([Parameter(Mandatory=$true)][string]$Port)
$ErrorActionPreference = 'Stop'
$stamp = [System.TimeZoneInfo]::ConvertTimeBySystemTimeZoneId([DateTimeOffset]::UtcNow, 'Turkey Standard Time').ToString('yyyyMMdd-HHmmss')
$folder = Join-Path (Split-Path $PSScriptRoot -Parent) "docs/test-results/phase5a-usb-$stamp"
[void][System.IO.Directory]::CreateDirectory($folder)
$raw = [System.Collections.Generic.List[byte]]::new()
$results = [System.Collections.Generic.List[object]]::new()
$serial = [System.IO.Ports.SerialPort]::new($Port,9600,[System.IO.Ports.Parity]::None,8,[System.IO.Ports.StopBits]::One)
$serial.DtrEnable = $true
$serial.ReadTimeout = 100
$serial.WriteTimeout = 1000
$failure = $null
function Capture {
    while ($serial.BytesToRead -gt 0) { $raw.Add([byte]$serial.ReadByte()) }
}
try {
    $serial.Open()
    $wait = [System.Diagnostics.Stopwatch]::StartNew()
    while ($wait.ElapsedMilliseconds -lt 6000) { Capture; Start-Sleep -Milliseconds 10 }
    $startup = [System.Text.Encoding]::ASCII.GetString($raw.ToArray())
    $results.Add([pscustomobject]@{kind='startup';received=$startup;hex=[Convert]::ToHexString($raw.ToArray())})
    if (!$startup.Contains("Mega Rust ready`r`n")) { throw 'Startup banner missing; no PING sent.' }
    if ($startup.Contains('HMI application enabled')) { throw 'Unexpected HMI activation message.' }
    for ($index=1; $index -le 3; $index++) {
        $offset=$raw.Count
        $serial.Write("PING`n")
        $wait.Restart()
        $reply=''
        while ($wait.ElapsedMilliseconds -lt 3000) {
            Capture
            $reply=[System.Text.Encoding]::ASCII.GetString($raw.ToArray(),$offset,$raw.Count-$offset)
            if ($reply.EndsWith("`r`n")) { break }
            Start-Sleep -Milliseconds 5
        }
        $pass=$reply -ceq "PONG`r`n"
        $results.Add([pscustomobject]@{kind='ping';index=$index;received=$reply;expected="PONG`r`n";pass=$pass})
        Write-Output "$(if ($pass) {'PASS'} else {'FAIL'}) PING $index -> $($reply.TrimEnd())"
        if (!$pass) { throw 'PING response mismatch or timeout; no retry.' }
    }
} catch { $failure=$_.Exception.Message; Write-Output "STOP: $failure" }
finally {
    if ($serial.IsOpen) { $serial.Close() }
    $serial.Dispose()
    [System.IO.File]::WriteAllBytes((Join-Path $folder 'serial-rx.bin'),$raw.ToArray())
    [System.IO.File]::WriteAllText((Join-Path $folder 'results.json'),([pscustomobject]@{port=$Port;baud=9600;failure=$failure;events=$results.ToArray()} | ConvertTo-Json -Depth 5))
    Write-Output "LOG_DIRECTORY=$folder"
}
if ($null -ne $failure) { exit 1 }
