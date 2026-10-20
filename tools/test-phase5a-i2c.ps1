param([Parameter(Mandatory=$true)][string]$Port)
$ErrorActionPreference = 'Stop'
$repo = Split-Path $PSScriptRoot -Parent
$stamp = [System.TimeZoneInfo]::ConvertTimeBySystemTimeZoneId([DateTimeOffset]::UtcNow, 'Turkey Standard Time').ToString('yyyyMMdd-HHmmss')
$folder = Join-Path $repo "docs/test-results/phase5a-i2c-$stamp"
[void][System.IO.Directory]::CreateDirectory($folder)
$raw = [System.Collections.Generic.List[byte]]::new()
$events = [System.Collections.Generic.List[object]]::new()
$serial = [System.IO.Ports.SerialPort]::new($Port,9600,[System.IO.Ports.Parity]::None,8,[System.IO.Ports.StopBits]::One)
$serial.DtrEnable = $true
$serial.ReadTimeout = 100
$serial.WriteTimeout = 1000
$failure = $null
function Receive-Bytes {
    while ($serial.BytesToRead -gt 0) { $raw.Add([byte]$serial.ReadByte()) }
}
try {
    $serial.Open()
    $startupWatch = [System.Diagnostics.Stopwatch]::StartNew()
    while ($startupWatch.ElapsedMilliseconds -lt 6000) { Receive-Bytes; Start-Sleep -Milliseconds 10 }
    $startup = [System.Text.Encoding]::ASCII.GetString($raw.ToArray())
    $events.Add([pscustomobject]@{kind='startup';port=$Port;hex=[Convert]::ToHexString($raw.ToArray());text=$startup})
    if (!$startup.Contains("Mega Rust ready`r`n")) { throw 'Mega startup identity not confirmed; no commands sent.' }
    $steps = @(
        @('PING','PONG'), @('STATUS','CLOSED/CLOSED'),
        @('OPEN 1','OPEN/CLOSED'), @('OPEN 2','OPEN/OPEN'),
        @('CLOSE 1','CLOSED/OPEN'), @('CLOSE 2','CLOSED/CLOSED'),
        @('STATUS','CLOSED/CLOSED'), @('PING','PONG')
    )
    foreach ($step in $steps) {
        $command = $step[0]
        $offset = $raw.Count
        $serial.Write("$command`n")
        $watch = [System.Diagnostics.Stopwatch]::StartNew()
        $received = ''
        while ($watch.ElapsedMilliseconds -lt 3000) {
            Receive-Bytes
            $received = [System.Text.Encoding]::ASCII.GetString($raw.ToArray(),$offset,$raw.Count-$offset)
            if ($received.EndsWith("`r`n")) { break }
            Start-Sleep -Milliseconds 5
        }
        $expectedState = $step[1]
        # This regression expects both simulated lockers closed initially.
        $expected = if ($expectedState -eq 'PONG') { @("PONG`r`n") }
        else { $parts = $expectedState.Split('/'); @("SIMULATED STATUS locker1=$($parts[0]) locker2=$($parts[1])`r`n") }
        $pass = $expected -ccontains $received
        $events.Add([pscustomobject]@{kind='command';command=$command;expected=$expected;received=$received;elapsed_ms=$watch.ElapsedMilliseconds;pass=$pass})
        Write-Output "$(if ($pass) {'PASS'} else {'FAIL'}) $command -> $($received.TrimEnd()) ($($watch.ElapsedMilliseconds) ms)"
        if (!$pass) { throw "Response mismatch or 3000 ms timeout at $command. No retry performed." }
    }
} catch {
    $failure = $_.Exception.Message
    Write-Output "STOP: $failure"
} finally {
    if ($serial.IsOpen) { $serial.Close() }
    $serial.Dispose()
    [System.IO.File]::WriteAllBytes((Join-Path $folder 'serial-rx.bin'),$raw.ToArray())
    [System.IO.File]::WriteAllText((Join-Path $folder 'serial-rx.hex'),[Convert]::ToHexString($raw.ToArray()))
    [System.IO.File]::WriteAllText((Join-Path $folder 'results.json'),([pscustomobject]@{port=$Port;baud=9600;format='8N1';failure=$failure;events=$events.ToArray()} | ConvertTo-Json -Depth 6))
    Write-Output "LOG_DIRECTORY=$folder"
}
if ($null -ne $failure) { exit 1 }
