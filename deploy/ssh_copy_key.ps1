$pubKey = Get-Content "$env:USERPROFILE\.ssh\id_rsa.pub"
$env:Path = "C:\Windows\System32\OpenSSH;$env:Path"

$psi = New-Object System.Diagnostics.ProcessStartInfo
$psi.FileName = "ssh"
$psi.Arguments = "-o StrictHostKeyChecking=no -o ConnectTimeout=10 debian@192.168.1.60 'mkdir -p ~/.ssh && echo `"$pubKey`" >> ~/.ssh/authorized_keys && chmod 700 ~/.ssh && chmod 600 ~/.ssh/authorized_keys && echo KEY_COPIED'"
$psi.RedirectStandardInput = $true
$psi.RedirectStandardOutput = $true
$psi.RedirectStandardError = $true
$psi.UseShellExecute = $false

$p = [System.Diagnostics.Process]::Start($psi)
Start-Sleep -Milliseconds 3000
if (-not $p.HasExited) {
    $p.StandardInput.WriteLine("9090")
    Start-Sleep -Milliseconds 5000
}
$output = $p.StandardOutput.ReadToEnd()
$err = $p.StandardError.ReadToEnd()
$p.WaitForExit(10000)
Write-Host "ExitCode: $($p.ExitCode)"
Write-Host "Output: $output"
Write-Host "Error: $err"