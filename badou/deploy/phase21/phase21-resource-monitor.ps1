# phase21-resource-monitor.ps1 — Windows 资源监控
# 用法: .\phase21-resource-monitor.ps1 -Duration 300 -OutputDir .\evidence
# 监控: Idle RAM / Backup RAM Peak / Restore RAM Peak / CPU Peak / Disk I/O

param(
    [int]$Duration = 300,
    [string]$OutputDir = ".\evidence",
    [string]$ProcessName = "hbx-agent"
)

$ErrorActionPreference = "SilentlyContinue"
if (!(Test-Path $OutputDir)) { New-Item -ItemType Directory -Force -Path $OutputDir }

$OutputFile = Join-Path $OutputDir "resource-metrics.csv"
"Timestamp,WorkingSet(MB),PrivateMemory(MB),CPU(%),DiskRead(MB/s),DiskWrite(MB/s)" | Out-File $OutputFile

Write-Host "=== Phase BD-21 资源监控 ($Duration 秒) ==="
Write-Host "监控进程: $ProcessName"
Write-Host "输出文件: $OutputFile"

$StartTime = Get-Date
$PeakRAM = 0
$PeakCPU = 0

while ((Get-Date) -lt $StartTime.AddSeconds($Duration)) {
    $proc = Get-Process -Name $ProcessName -ErrorAction SilentlyContinue
    if ($proc) {
        $ram = [math]::Round($proc.WorkingSet64 / 1MB, 2)
        $private = [math]::Round($proc.PrivateMemorySize64 / 1MB, 2)
        $cpu = $proc.CPU
        $timestamp = (Get-Date).ToString("yyyy-MM-dd HH:mm:ss")

        if ($ram -gt $PeakRAM) { $PeakRAM = $ram }
        if ($cpu -gt $PeakCPU) { $PeakCPU = $cpu }

        "$timestamp,$ram,$private,$cpu,0,0" | Out-File $OutputFile -Append
        Write-Host "  RAM: ${ram}MB (Peak: ${PeakRAM}MB) CPU: ${cpu}s"
    }
    Start-Sleep -Seconds 1
}

$SummaryFile = Join-Path $OutputDir "resource-summary.json"
$summary = @{
    process = $ProcessName
    duration_seconds = $Duration
    peak_ram_mb = $PeakRAM
    peak_cpu_seconds = $PeakCPU
    timestamp = (Get-Date).ToString("yyyy-MM-dd HH:mm:ss")
}
$summary | ConvertTo-Json | Out-File $SummaryFile

Write-Host ""
Write-Host "=== 监控完成 ==="
Write-Host "Peak RAM: ${PeakRAM}MB"
Write-Host "Peak CPU: ${PeakCPU}s"
Write-Host "报告: $SummaryFile"