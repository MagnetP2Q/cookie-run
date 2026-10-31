[CmdletBinding()]
param(
    [Parameter(Mandatory=$true)][int]$AppProcessId,
    [switch]$AllowFixtureTestProcess,
    [int[]]$EmulatorProcessIds = @(),
    [int[]]$AdbServerProcessIds = @(),
    [ValidateRange(1,60)][int]$IntervalSeconds = 60,
    [ValidateRange(1,2592000)][int]$DurationSeconds = 2592000,
    [string]$OutputRoot = (Join-Path $env:LOCALAPPDATA 'CookieRunBotMetrics'),
    [ValidateRange(1,1024)][int]$MaxTotalMiB = 256
)
$ErrorActionPreference = 'Stop'
[System.Threading.Thread]::CurrentThread.CurrentCulture = [System.Globalization.CultureInfo]::InvariantCulture
[System.Threading.Thread]::CurrentThread.CurrentUICulture = [System.Globalization.CultureInfo]::InvariantCulture
$initial = Get-Process -Id $AppProcessId -ErrorAction Stop
$fixtureRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\src-tauri\target\release\deps')) + [IO.Path]::DirectorySeparatorChar
$fixturePath = $null
try { $fixturePath = $initial.Path } catch {}
$isFixture = $AllowFixtureTestProcess -and $initial.ProcessName -match '^cookie_run_lib-[0-9a-f]{16}$' -and $fixturePath -and $fixturePath.StartsWith($fixtureRoot, [StringComparison]::OrdinalIgnoreCase)
if ($initial.ProcessName -ne 'cookie-run' -and !$isFixture) { throw "PID $AppProcessId is not cookie-run or an allowed release fixture test process." }
$primaryGroup = if ($isFixture) { 'fixture' } else { 'app' }
$expectedName = $initial.ProcessName
$appStart = $initial.StartTime.ToUniversalTime().Ticks
$emulatorStarts=@{}
foreach($emulatorPid in $EmulatorProcessIds){ $ep=Get-Process -Id $emulatorPid -ErrorAction Stop; $emulatorStarts[$emulatorPid]=$ep.StartTime.ToUniversalTime().Ticks }
$adbStarts=@{}
foreach($adbPid in $AdbServerProcessIds){$adbProc=Get-Process -Id $adbPid -ErrorAction Stop;if($adbProc.ProcessName -ne 'adb'){throw "PID $adbPid is not adb"};$adbStarts[$adbPid]=$adbProc.StartTime.ToUniversalTime().Ticks}
# CIM is essential for ancestry; stop rather than silently misattribute processes.
try { $null = Get-CimInstance Win32_Process -Filter "ProcessId = $AppProcessId" -ErrorAction Stop } catch { throw "Cannot query process ancestry: $($_.Exception.Message)" }
$session = Join-Path $OutputRoot ('session-' + [DateTime]::UtcNow.ToString('yyyyMMddTHHmmssZ') + '-' + [Guid]::NewGuid().ToString('N').Substring(0,8))
New-Item -ItemType Directory -Path $session -Force | Out-Null
$path = Join-Path $session 'metrics.csv'
$utf8 = New-Object System.Text.UTF8Encoding $false
$stream = New-Object System.IO.StreamWriter($path,$false,$utf8)
$header='timestamp_utc,elapsed_seconds,sample_gap_seconds,group,pid,process_start_utc,name,private_bytes,working_set_bytes,handles,threads,cpu_seconds,cpu_cores,status'
$stream.WriteLine($header);$stream.Flush()
$maxBytes = [long]$MaxTotalMiB * 1MB
$previous = @{}; $startWall=[DateTime]::UtcNow; $lastWall=$null; $deadline=$startWall.AddSeconds($DurationSeconds); $appMissing=$false
try {
 while ([DateTime]::UtcNow -lt $deadline) {
  $now=[DateTime]::UtcNow; $elapsed=($now-$startWall).TotalSeconds; $gap=if($lastWall){($now-$lastWall).TotalSeconds}else{0}; $lastWall=$now
  $snapshot=@(Get-CimInstance Win32_Process -ErrorAction Stop)
  $byId=@{};foreach($p in $snapshot){$byId[[int]$p.ProcessId]=$p}
  $appAlive=$false
  try { $app=Get-Process -Id $AppProcessId -ErrorAction Stop; $appAlive=($app.ProcessName -eq $expectedName -and $app.StartTime.ToUniversalTime().Ticks -eq $appStart) } catch {}
  $groups=@{}
  if($appAlive){$groups[$AppProcessId]=$primaryGroup}
  foreach($emulatorPid in $EmulatorProcessIds){try{$ep=Get-Process -Id $emulatorPid -ErrorAction Stop;if($ep.StartTime.ToUniversalTime().Ticks -eq $emulatorStarts[$emulatorPid]){$groups[$emulatorPid]='emulator'}}catch{}}
  foreach($adbPid in $AdbServerProcessIds){try{$adbProc=Get-Process -Id $adbPid -ErrorAction Stop;if($adbProc.ProcessName -eq 'adb' -and $adbProc.StartTime.ToUniversalTime().Ticks -eq $adbStarts[$adbPid]){$groups[$adbPid]='adb_shared'}}catch{}}
  # Descendants only. Shared/reparented WebView2 and short-lived ADB may be missed.
  for($pass=0;$pass -lt 12;$pass++){
   $changed=$false
   foreach($p in $snapshot){$id=[int]$p.ProcessId;$parent=[int]$p.ParentProcessId;if(!$groups.ContainsKey($id) -and $groups.ContainsKey($parent)){
    $name=[string]$p.Name;$ancestor=$groups[$parent]
    if($ancestor -eq 'emulator'){$groups[$id]='emulator'}
    elseif($ancestor -eq 'adb_shared'){$groups[$id]='adb_shared'}
    elseif($name -ieq 'msedgewebview2.exe' -or $ancestor -eq 'webview2'){$groups[$id]='webview2'}
    elseif($name -ieq 'adb.exe'){$groups[$id]='adb_child'}
    else {$groups[$id]='app_child'}
    $changed=$true
   }}
   if(!$changed){break}
  }
  if(!$appAlive){$appMissing=$true;$stream.WriteLine(('{0},{1:F3},{2:F3},{3},{4},,,,,,,,,app_missing' -f $now.ToString('o'),$elapsed,$gap,$primaryGroup,$AppProcessId))}
  foreach($id in @($groups.Keys | Sort-Object)){
   try {
    $proc=Get-Process -Id $id -ErrorAction Stop;$started=$proc.StartTime.ToUniversalTime();$key="${id}:$($started.Ticks)";$cpu=[double]$proc.CPU;$cores=''
    if($previous.ContainsKey($key)){$delta=($now-$previous[$key].time).TotalSeconds;if($delta -gt 0){$cores=('{0:F4}' -f (($cpu-$previous[$key].cpu)/$delta))}}
    $previous[$key]=@{time=$now;cpu=$cpu};$name=$proc.ProcessName -replace '[,\r\n]','_'
    $stream.WriteLine(('{0},{1:F3},{2:F3},{3},{4},{5},{6},{7},{8},{9},{10},{11:F4},{12},ok' -f $now.ToString('o'),$elapsed,$gap,$groups[$id],$id,$started.ToString('o'),$name,$proc.PrivateMemorySize64,$proc.WorkingSet64,$proc.HandleCount,$proc.Threads.Count,$cpu,$cores))
   } catch { $stream.WriteLine(('{0},{1:F3},{2:F3},{3},{4},,,,,,,,,unavailable' -f $now.ToString('o'),$elapsed,$gap,$groups[$id],$id)) }
  }
  # Drop vanished PIDs so monitoring memory itself stays bounded.
  $validKeys=@{};foreach($id in $groups.Keys){try{$p=Get-Process -Id $id -ErrorAction Stop;$validKeys["${id}:$($p.StartTime.ToUniversalTime().Ticks)"]=$true}catch{}}
  foreach($key in @($previous.Keys)){if(!$validKeys.ContainsKey($key)){$previous.Remove($key)}}
  $stream.Flush()
  if($stream.BaseStream.Length -ge $maxBytes){throw "Metrics file reached the $MaxTotalMiB MiB limit; collection stopped."}
  $remaining=($deadline-[DateTime]::UtcNow).TotalSeconds;if($remaining -le 0){break}
  Start-Sleep -Milliseconds ([int]([Math]::Min($IntervalSeconds,$remaining)*1000))
 }
 $stream.WriteLine(('{0},{1:F3},0,monitor,,,,,,,,,,{2}' -f [DateTime]::UtcNow.ToString('o'),([DateTime]::UtcNow-$startWall).TotalSeconds,($(if($appMissing){'completed_with_app_missing'}else{'completed'}))))
} finally { $stream.Dispose() }
Write-Output "Metrics: $path"
