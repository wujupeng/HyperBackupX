param(
    [Parameter(Mandatory=$true)][string]$AgentProcessName,
    [Parameter(Mandatory=$true)][string]$Phase,
    [int]$DurationSec = 30,
    [string]$OutputDir = "C:\evidence"
)

if (-not (Test-Path $OutputDir)) { New-Item -ItemType Directory -Force -Path $OutputDir | Out-Null }

$counters = @(
    "\Process($AgentProcessName)\Working Set Peak"
    "\Process($AgentProcessName)\% Processor Time"
    "\Process($AgentProcessName)\IO Read Bytes/sec"
    "\Process($AgentProcessName)\IO Write Bytes/sec"
)

$countersFile = Join-Path $OutputDir "counters.txt"
$counters | Out-File -FilePath $countersFile -Encoding ASCII

$csvFile = Join-Path $OutputDir "typeperf-$Phase.csv"
$interval = 1
$samples = $DurationSec / $interval

Write-Host "[monitor] Phase=$Phase Process=$AgentProcessName Duration=${DurationSec}s Samples=$samples"

try {
    $proc = Get-Process -Name $AgentProcessName -ErrorAction Stop
    Write-Host "[monitor] Process found PID=$($proc.Id) RSS=$([math]::Round($proc.WorkingSet64/1MB,2))MB"
} catch {
    Write-Host "[monitor] WARNING: Process '$AgentProcessName' not found, typeperf will record empty counters"
}

& typeperf -cf $countersFile -o $csvFile -sc $samples -si $interval 2>&1 | Out-Null

$peakRss = 0.0
$cpuPeak = 0.0
$ioReadAvg = 0.0
$ioWriteAvg = 0.0
$validRows = 0

if (Test-Path $csvFile) {
    $lines = Get-Content $csvFile | Select-Object -Skip 2
    foreach ($line in $lines) {
        $cols = $line -split '","'
        if ($cols.Count -ge 5) {
            $vals = $cols[1..4] | ForEach-Object { $_.Trim('"') }
            try {
                $rss = [double]::Parse($vals[0], [System.Globalization.CultureInfo]::InvariantCulture)
                $cpu = [double]::Parse($vals[1], [System.Globalization.CultureInfo]::InvariantCulture)
                $ioR = [double]::Parse($vals[2], [System.Globalization.CultureInfo]::InvariantCulture)
                $ioW = [double]::Parse($vals[3], [System.Globalization.CultureInfo]::InvariantCulture)
                if ($rss -gt $peakRss) { $peakRss = $rss }
                if ($cpu -gt $cpuPeak) { $cpuPeak = $cpu }
                $ioReadAvg += $ioR
                $ioWriteAvg += $ioW
                $validRows++
            } catch {}
        }
    }
}

if ($validRows -gt 0) {
    $ioReadAvg /= $validRows
    $ioWriteAvg /= $validRows
}

$summary = @{
    phase = $Phase
    peak_rss_mb = [math]::Round($peakRss / 1MB, 2)
    cpu_peak_percent = [math]::Round($cpuPeak, 2)
    io_read_mbps = [math]::Round($ioReadAvg / 1MB, 2)
    io_write_mbps = [math]::Round($ioWriteAvg / 1MB, 2)
    monitor_source = "TYPEPERF"
    monitor_evidence = $csvFile
    timestamp = (Get-Date -Format "yyyy-MM-ddTHH:mm:ssZ")
    valid_samples = $validRows
}

$jsonFile = Join-Path $OutputDir "summary-$Phase.json"
$summary | ConvertTo-Json -Depth 3 | Out-File -FilePath $jsonFile -Encoding UTF8

Write-Host "[monitor] Summary: peak_rss_mb=$($summary.peak_rss_mb) cpu_peak=$($summary.cpu_peak_percent)% io_read=$($summary.io_read_mbps)MB/s io_write=$($summary.io_write_mbps)MB/s"
Write-Host "[monitor] JSON saved: $jsonFile"