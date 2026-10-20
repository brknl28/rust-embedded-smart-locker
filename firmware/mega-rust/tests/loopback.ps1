param([string]$PortName = 'COM3')

# Requires the temporary UART bridge and a D18-to-D19 jumper, with Nextion absent.
# Does not flash firmware or send Nextion commands. Failures stop the entire run.
$ErrorActionPreference = 'Stop'

function Test-Payload {
    param($Serial, [byte[]]$Payload, [string]$Label)
    $actual = [byte[]]::new($Payload.Length)
    $Serial.Write($Payload, 0, $Payload.Length)
    $offset = 0
    $deadline = [DateTime]::UtcNow.AddSeconds($Payload.Length / 960.0 + 8)
    while ($offset -lt $actual.Length) {
        if ([DateTime]::UtcNow -gt $deadline) { throw "$Label total receive deadline exceeded" }
        $offset += $Serial.Read($actual, $offset, $actual.Length - $offset)
    }
    for ($i = 0; $i -lt $Payload.Length; $i++) {
        if ($Payload[$i] -ne $actual[$i]) {
            throw "$Label byte mismatch at $i : expected=$($Payload[$i]) actual=$($actual[$i])"
        }
    }
    Start-Sleep -Milliseconds 100
    if ($Serial.BytesToRead -ne 0) { throw "$Label unexpected extra receive bytes: $($Serial.BytesToRead)" }
    $sha = [System.Security.Cryptography.SHA256]::Create()
    try { $hash = [BitConverter]::ToString($sha.ComputeHash($actual)).Replace('-', '') }
    finally { $sha.Dispose() }
    Write-Output "PASS $Label bytes=$offset SHA256=$hash"
}

$allBytes = [byte[]](0..255)
$repeated = [byte[]]::new(4096)
$reverse = [byte[]]::new(4096)
$long = [byte[]]::new(16384)
for ($i = 0; $i -lt 4096; $i++) {
    $repeated[$i] = $i % 256
    $reverse[$i] = 255 - ($i % 256)
}
$rng = [Random]::new(2560)
$rng.NextBytes($long)

for ($cycle = 1; $cycle -le 3; $cycle++) {
    $serial = [System.IO.Ports.SerialPort]::new($PortName, 9600, [System.IO.Ports.Parity]::None, 8, [System.IO.Ports.StopBits]::One)
    $serial.Handshake = [System.IO.Ports.Handshake]::None
    $serial.DtrEnable = $true
    $serial.ReadBufferSize = 65536
    $serial.ReadTimeout = 3000
    $serial.WriteTimeout = 30000
    try {
        $serial.Open()
        Start-Sleep -Seconds 2 # permit a possible auto-reset/bootloader interval
        if ($serial.BytesToRead -ne 0) { throw "Cycle $cycle unexpected startup bytes" }
        Write-Output "OPEN cycle=$cycle port=$PortName baud=9600 8N1 DTR=true"
        Test-Payload $serial $allBytes "cycle${cycle}-all-byte-values"
        if ($cycle -eq 1) {
            Test-Payload $serial $repeated 'repeated-4096'
            Test-Payload $serial $reverse 'reverse-4096'
            Test-Payload $serial $long 'seeded-binary-16384'
        }
        Write-Output "CLOSE cycle=$cycle"
    } finally {
        if ($serial.IsOpen) { $serial.Close() }
        $serial.Dispose()
    }
}
Write-Output 'ALL LOOPBACK TESTS PASSED: 25344 bytes compared, 3 port-open cycles'
