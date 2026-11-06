# Passive test: never opens the scope. Reports arrival, then how long it stays on the bus untouched.
$id = 'USB\VID_049F&PID_505E\CN5524029091177'
$prev = $null
$since = $null
$lastReport = $null
while ($true) {
    $d = Get-PnpDevice -InstanceId $id -ErrorAction SilentlyContinue
    $present = [bool]($d -and $d.Present)
    $now = Get-Date
    if ($present -ne $prev) {
        if ($present) {
            $since = $now
            $lastReport = $now
            [Console]::Out.WriteLine("$($now.ToString('HH:mm:ss')) PRESENT (idle test started, nothing will talk to the scope)")
        } elseif ($since) {
            [Console]::Out.WriteLine("$($now.ToString('HH:mm:ss')) DROPPED while idle after $([int]($now - $since).TotalSeconds) s")
        } else {
            [Console]::Out.WriteLine("$($now.ToString('HH:mm:ss')) waiting for scope (absent)")
        }
        [Console]::Out.Flush()
        $prev = $present
    }
    if ($present -and ($now - $lastReport).TotalSeconds -ge 60) {
        [Console]::Out.WriteLine("$($now.ToString('HH:mm:ss')) still present idle: $([int]($now - $since).TotalSeconds) s")
        [Console]::Out.Flush()
        $lastReport = $now
    }
    Start-Sleep -Milliseconds 700
}
