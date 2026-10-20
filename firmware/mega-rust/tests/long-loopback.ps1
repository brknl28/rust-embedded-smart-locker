param(
    [string]$PortName = 'COM3',
    [int]$PayloadSize = 824432,
    [int]$Cycles = 2
)

# Owner-approved physical test only: Nextion must be absent; D18/D19 jumpered.
# Raw bytes only. No Nextion protocol, baud switching, firmware build or flashing.
$ErrorActionPreference = 'Stop'
if ($PortName -ne 'COM3' -or $PayloadSize -ne 824432 -or $Cycles -ne 2) {
    throw 'This checkpoint is restricted to COM3, 824432 bytes, two cycles.'
}
$repoPath = (Resolve-Path (Join-Path $PSScriptRoot '../../..')).Path
$outputPath = Join-Path $repoPath 'tools/nextion-upload/target'
New-Item -ItemType Directory -Path $outputPath -Force | Out-Null
$logPath = Join-Path $outputPath 'phase3d-physical-loopback.log'
$resultPath = Join-Path $outputPath 'phase3d-physical-loopback.json'
if ((Test-Path -LiteralPath $logPath) -or (Test-Path -LiteralPath $resultPath)) {
    throw 'Evidence files already exist; preserve them before authorizing another run.'
}

function Write-Evidence([string]$Line) {
    $record = "$(Get-Date -Format 'yyyy-MM-ddTHH:mm:sszzz') $Line"
    Add-Content -LiteralPath $logPath -Value $record -Encoding utf8
    Write-Output $record
}

function Get-BytesHash([byte[]]$Bytes) {
    $sha = [System.Security.Cryptography.SHA256]::Create()
    try { return [BitConverter]::ToString($sha.ComputeHash($Bytes)).Replace('-', '') }
    finally { $sha.Dispose() }
}

function Read-Available($Serial, [byte[]]$Payload, [byte[]]$Actual, $State) {
    while ($Serial.BytesToRead -gt 0) {
        $available = $Serial.BytesToRead
        if ($available -gt $Actual.Length - $State.Received) {
            throw "Unexpected extra receive bytes at offset $($State.Received)"
        }
        $offset = $State.Received
        $count = $Serial.Read($Actual, $offset, $available)
        if ($count -eq 0) { throw 'Serial read returned zero bytes' }
        for ($j = $offset; $j -lt $offset + $count; $j++) {
            if ($Payload[$j] -ne $Actual[$j]) {
                throw "Byte mismatch at offset ${j}: expected=$($Payload[$j]) actual=$($Actual[$j])"
            }
        }
        $State.Received += $count
        $State.LastReceiveMs = $State.Clock.ElapsedMilliseconds
        if ($State.Received -gt $State.Sent) { throw 'Received bytes exceed submitted bytes' }
    }
}

$payload = [byte[]]::new($PayloadSize)
# Deterministic cycling 00..FF; contains every byte value. Not TFT file contents.
for ($i = 0; $i -lt $payload.Length; $i++) { $payload[$i] = $i % 256 }
$expectedHash = Get-BytesHash $payload
$results = [System.Collections.Generic.List[object]]::new()
$run = [ordered]@{
    Port = $PortName; Baud = 9600; DataBits = 8; Parity = 'None'; StopBits = 1
    PayloadSize = $PayloadSize; Pattern = 'cycling 00..FF'
    ExpectedSHA256 = $expectedHash; ChunkSize = 16; ExtraGapMs = 10
    Started = (Get-Date).ToString('o'); Status = 'RUNNING'; Cycles = $results; Error = $null
}
Write-Evidence "START port=$PortName baud=9600 8N1 bytes=$PayloadSize cycles=$Cycles pattern=00..FF SHA256=$expectedHash"
try {
    for ($cycle = 1; $cycle -le $Cycles; $cycle++) {
        $serial = [System.IO.Ports.SerialPort]::new($PortName, 9600, [System.IO.Ports.Parity]::None, 8, [System.IO.Ports.StopBits]::One)
        $serial.Handshake = [System.IO.Ports.Handshake]::None
        $serial.DtrEnable = $true
        $serial.ReadBufferSize = 65536
        $serial.WriteBufferSize = 4096
        $serial.ReadTimeout = 250
        $serial.WriteTimeout = 2000
        $state = @{ Sent = 0; Received = 0; LastReceiveMs = 0; Clock = [System.Diagnostics.Stopwatch]::new() }
        try {
            $serial.Open()
            Start-Sleep -Seconds 3
            if ($serial.BytesToRead -ne 0) { throw "Cycle $cycle unexpected startup bytes: $($serial.BytesToRead)" }
            Write-Evidence "OPEN cycle=$cycle DTR=true boot_wait=3s startup_bytes=0"
            $actual = [byte[]]::new($payload.Length)
            $state.Clock.Start()
            $nextProgressMs = 30000L
            for ($offset = 0; $offset -lt $payload.Length; $offset += 16) {
                Read-Available $serial $payload $actual $state
                $count = [Math]::Min(16, $payload.Length - $offset)
                $chunkStart = $state.Clock.ElapsedMilliseconds
                $serial.Write($payload, $offset, $count)
                $state.Sent += $count
                # Bound queued host output. Queue-empty is not proof of AVR receipt.
                while ($serial.BytesToWrite -gt 0) {
                    Read-Available $serial $payload $actual $state
                    if ($state.Clock.ElapsedMilliseconds - $chunkStart -gt 2000) { throw 'Output queue drain timeout' }
                    Start-Sleep -Milliseconds 2
                }
                $due = $chunkStart + [Math]::Ceiling($count * 10000.0 / 9600.0) + 10
                while ($state.Clock.ElapsedMilliseconds -lt $due) {
                    Read-Available $serial $payload $actual $state
                    $remaining = $due - $state.Clock.ElapsedMilliseconds
                    if ($remaining -gt 0) { Start-Sleep -Milliseconds ([int][Math]::Min(5, $remaining)) }
                }
                Read-Available $serial $payload $actual $state
                if ($state.Sent -gt $state.Received -and $state.Clock.ElapsedMilliseconds - $state.LastReceiveMs -gt 3000) {
                    throw "Receive stalled: sent=$($state.Sent) received=$($state.Received)"
                }
                if ($state.Clock.ElapsedMilliseconds -ge $nextProgressMs) {
                    Write-Evidence "PROGRESS cycle=$cycle sent=$($state.Sent)/$PayloadSize received=$($state.Received) percent=$([Math]::Round(100.0 * $state.Received / $PayloadSize, 2)) elapsed_s=$([Math]::Round($state.Clock.Elapsed.TotalSeconds, 1))"
                    $nextProgressMs = $state.Clock.ElapsedMilliseconds + 30000
                }
                if ($state.Clock.Elapsed.TotalMinutes -gt 45) { throw "Cycle $cycle exceeded 45-minute bounded deadline" }
            }
            $tailDeadline = $state.Clock.ElapsedMilliseconds + 3000
            while ($state.Received -lt $payload.Length) {
                Read-Available $serial $payload $actual $state
                if ($state.Clock.ElapsedMilliseconds -gt $tailDeadline) { throw 'Final receive timeout' }
                Start-Sleep -Milliseconds 2
            }
            Start-Sleep -Milliseconds 500
            if ($serial.BytesToRead -ne 0) { throw "Cycle $cycle extra trailing bytes=$($serial.BytesToRead)" }
            $actualHash = Get-BytesHash $actual
            if ($expectedHash -ne $actualHash) { throw "Cycle $cycle SHA256 mismatch" }
            $state.Clock.Stop()
            $result = [ordered]@{
                Cycle = $cycle; Status = 'PASS'; Sent = $state.Sent; Received = $state.Received
                ByteMismatches = 0; ExtraBytes = 0; SentSHA256 = $expectedHash; ReceivedSHA256 = $actualHash
                ElapsedSeconds = [Math]::Round($state.Clock.Elapsed.TotalSeconds, 3)
            }
            $results.Add($result)
            Write-Evidence "PASS cycle=$cycle sent=$($state.Sent) received=$($state.Received) byte_mismatches=0 extra_bytes=0 SHA256=$actualHash elapsed_s=$($result.ElapsedSeconds)"
        } finally {
            if ($serial.IsOpen) { $serial.Close() }
            $serial.Dispose()
            Write-Evidence "CLOSED cycle=$cycle sent=$($state.Sent) received=$($state.Received)"
        }
        if ($cycle -lt $Cycles) { Start-Sleep -Seconds 1 }
    }
    $run.Status = 'PASS'
    Write-Evidence "ALL LONG LOOPBACK TESTS PASSED total_bytes=$($PayloadSize * $Cycles) opens=$Cycles"
} catch {
    $run.Status = 'FAIL'
    $run.Error = $_.Exception.Message
    Write-Evidence "STOP $($run.Error); no retry, flash or Nextion command"
    throw
} finally {
    $run['Finished'] = (Get-Date).ToString('o')
    $run | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath $resultPath -Encoding utf8
}
