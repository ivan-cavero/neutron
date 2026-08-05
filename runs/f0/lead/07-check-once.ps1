$ErrorActionPreference = 'Stop'
$out = 'C:\Users\ivang\neutron\runs\f0\lead\out'
$stamp = Get-Date -Format 'yyyyMMdd-HHmmss'
orca orchestration check --types worker_done,escalation,question --json 2>&1 | Out-File -FilePath "$out\check-once-$stamp.json" -Encoding utf8
Write-Output "EXIT:$LASTEXITCODE CHECKFILE:check-once-$stamp.json"
