# Pregenerate chunks around several centers so status becomes full / light.
# Centers: spawn (0,0), deep dark area (6,-2), and offset multi-biome samples.
$ErrorActionPreference = "Stop"
$root = $PSScriptRoot
Set-Location $root

# Ensure eula
if (-not (Test-Path eula.txt)) { "eula=true" | Set-Content eula.txt }

$jar = if (Test-Path "server.jar") { "server.jar" } else { "versions\26.2\server-26.2.jar" }
$log = Join-Path $root "pregen.log"
if (Test-Path $log) { Remove-Item $log }

Write-Host "Starting vanilla 26.2 server for pregen..."
$psi = New-Object System.Diagnostics.ProcessStartInfo
$psi.FileName = "java"
$psi.Arguments = "-Xms2G -Xmx2G -XX:+AlwaysPreTouch -jar $jar nogui"
$psi.WorkingDirectory = $root
$psi.RedirectStandardInput = $true
$psi.RedirectStandardOutput = $true
$psi.RedirectStandardError = $true
$psi.UseShellExecute = $false
$psi.CreateNoWindow = $true
$p = New-Object System.Diagnostics.Process
$p.StartInfo = $psi
[void]$p.Start()

$stdout = $p.StandardOutput
$stdin = $p.StandardInput
$sb = New-Object System.Text.StringBuilder
$done = $false
$deadline = (Get-Date).AddMinutes(8)

while (-not $p.HasExited -and (Get-Date) -lt $deadline) {
  while (-not $stdout.EndOfStream -and $stdout.Peek() -ge 0) {
    $line = $stdout.ReadLine()
    [void]$sb.AppendLine($line)
    if ($line -match "Done \(") { $done = $true }
  }
  if ($done) { break }
  Start-Sleep -Milliseconds 200
}

if (-not $done) {
  Write-Host "Server failed to start; dumping log"
  $sb.ToString() | Set-Content $log
  if (-not $p.HasExited) { $p.Kill() }
  exit 1
}

Write-Host "Server ready. Forceloading chunk squares..."
# Multiple 17x17 chunk squares (~spawn, deep dark, +offsets for biomes)
$centers = @(
  @(0, 0),
  @(6, -2),
  @(32, 0),
  @(-32, 16),
  @(0, 48),
  @(64, -32),
  @(-48, -48)
)
$r = 8  # radius in chunks => 17x17 per center
foreach ($c in $centers) {
  $x0 = ($c[0] - $r) * 16
  $z0 = ($c[1] - $r) * 16
  $x1 = ($c[0] + $r) * 16
  $z1 = ($c[1] + $r) * 16
  $cmd = "execute in minecraft:overworld run forceload add $x0 $z0 $x1 $z1"
  Write-Host "  $cmd"
  $stdin.WriteLine($cmd)
  $stdin.Flush()
  Start-Sleep -Seconds 2
}

Write-Host "Waiting for generation (up to 10 min)..."
$waitUntil = (Get-Date).AddMinutes(10)
$lastLen = 0
while ((Get-Date) -lt $waitUntil -and -not $p.HasExited) {
  while (-not $stdout.EndOfStream -and $stdout.Peek() -ge 0) {
    $line = $stdout.ReadLine()
    [void]$sb.AppendLine($line)
  }
  # poke ticking so force-loaded chunks generate
  $stdin.WriteLine("execute in minecraft:overworld run time query daytime")
  $stdin.Flush()
  Start-Sleep -Seconds 15
  # progress: count region file sizes
  $reg = Get-ChildItem "world\dimensions\minecraft\overworld\region\*.mca" -ErrorAction SilentlyContinue
  $sz = ($reg | Measure-Object -Property Length -Sum).Sum
  if ($sz -ne $lastLen) {
    Write-Host ("  region bytes={0:N0}" -f $sz)
    $lastLen = $sz
  }
}

Write-Host "Saving and stopping..."
$stdin.WriteLine("save-all flush")
$stdin.Flush()
Start-Sleep -Seconds 5
$stdin.WriteLine("stop")
$stdin.Flush()
$p.WaitForExit(120000)
$sb.ToString() | Set-Content $log
Write-Host "Pregen finished. exit=$($p.ExitCode) log=$log"
