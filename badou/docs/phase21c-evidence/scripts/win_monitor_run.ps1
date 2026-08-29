$ErrorActionPreference = "Stop"
$exePath = "C:\agent-sim\target\release\badou-agent-sim.exe"
$outDir = "C:\evidence"
New-Item -ItemType Directory -Force -Path $outDir | Out-Null
$phases = @("idle","backup","incremental","restore")
$duration = 10
$results = @()
foreach ($phase in $phases) {
    Write-Output "=== Phase: $phase ==="
    $proc = Start-Process -FilePath $exePath -ArgumentList @($phase,$duration) -PassThru -RedirectStandardError "$outDir\${phase}_stderr.txt"
    $samples = @()
    while (-not $proc.HasExited) {
        $p = Get-Process -Id $proc.Id -ErrorAction SilentlyContinue
        if ($p) {
            $samples += [PSCustomObject]@{
                rss_mb = [math]::Round($p.WorkingSet64 / 1MB, 2)
                priv_mb = [math]::Round($p.PrivateMemorySize64 / 1MB, 2)
                cpu_sec = $p.CPU
                handles = $p.HandleCount
                threads = $p.Threads.Count
            }
        }
        Start-Sleep -Milliseconds 500
    }
    $peakRSS = ($samples | Measure-Object -Property rss_mb -Maximum).Maximum
    $peakPriv = ($samples | Measure-Object -Property priv_mb -Maximum).Maximum
    $peakHandles = ($samples | Measure-Object -Property handles -Maximum).Maximum
    $peakThreads = ($samples | Measure-Object -Property threads -Maximum).Maximum
    $cpuStart = $samples[0].cpu_sec
    $cpuEnd = $samples[$samples.Count - 1].cpu_sec
    $cpuTotal = if ($cpuEnd -ne $null -and $cpuStart -ne $null) { $cpuEnd - $cpuStart } else { 0 }
    $cpuPct = [math]::Round(($cpuTotal / $duration) * 100, 2)
    $samples | Export-Csv -Path "$outDir\${phase}_metrics.csv" -NoTypeInformation
    $r = [PSCustomObject]@{
        phase = $phase
        duration_sec = $duration
        peak_rss_mb = $peakRSS
        peak_private_mb = $peakPriv
        cpu_avg_percent = $cpuPct
        peak_handle_count = $peakHandles
        peak_thread_count = $peakThreads
        sample_count = $samples.Count
        exit_code = $proc.ExitCode
    }
    $results += $r
    Write-Output "  peak_rss=$peakRSS MB cpu=$cpuPct% handles=$peakHandles threads=$peakThreads"
    Start-Sleep -Seconds 1
}
$summary = [PSCustomObject]@{
    machine = "10.1.8.107"
    os = "Windows 11 24H2"
    build = "26100"
    ram_mb = 8191
    timestamp = (Get-Date).ToString("o")
    results = $results
}
$summary | ConvertTo-Json -Depth 5 | Out-File -FilePath "$outDir\summary.json" -Encoding UTF8
Write-Output "=== DONE ==="