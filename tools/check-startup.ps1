param(
    [Parameter(Mandatory)][string]$ProfilePath,
    [Parameter(Mandatory)][string]$GameExe,
    [string]$Me3Exe='me3',
    [ValidateRange(10,120)][int]$Seconds=30,
    [string]$OutputRoot="$PSScriptRoot\..\dist\startup-check"
)
# Starts a real game process and closes only that process after the check.
# The game can update the selected profile's save as it normally does at startup.
$ErrorActionPreference='Stop'
Add-Type -TypeDefinition @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;
public static class MinimapStartupWindows {
    private delegate bool Callback(IntPtr window, IntPtr data);
    [DllImport("user32.dll")] private static extern bool EnumWindows(Callback callback, IntPtr data);
    [DllImport("user32.dll")] private static extern uint GetWindowThreadProcessId(IntPtr window, out uint process);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] private static extern int GetWindowText(IntPtr window, StringBuilder text, int count);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] private static extern int GetClassName(IntPtr window, StringBuilder text, int count);
    [DllImport("user32.dll")] private static extern bool IsWindowVisible(IntPtr window);
    [DllImport("user32.dll", SetLastError=true)] private static extern IntPtr SendMessageTimeout(IntPtr window, uint message, UIntPtr wparam, IntPtr lparam, uint flags, uint timeout, out UIntPtr result);
    public static IntPtr FindGameWindow(uint process) {
        IntPtr found=IntPtr.Zero;
        EnumWindows((window, data) => {
            uint owner; GetWindowThreadProcessId(window, out owner);
            if(owner != process || !IsWindowVisible(window)) return true;
            var title=new StringBuilder(512); GetWindowText(window,title,title.Capacity);
            if(title.ToString()=="ELDEN RING" || title.ToString()=="ELDEN RING\u2122") { found=window; return false; }
            return true;
        },IntPtr.Zero);
        return found;
    }
    public static bool Responds(IntPtr window) {
        UIntPtr result;
        return window!=IntPtr.Zero && SendMessageTimeout(window,0,UIntPtr.Zero,IntPtr.Zero,2,1000,out result)!=IntPtr.Zero;
    }
    public static string[] Describe(uint process) {
        var windows=new List<string>();
        EnumWindows((window, data) => {
            uint owner; GetWindowThreadProcessId(window, out owner);
            if(owner != process) return true;
            var title=new StringBuilder(512); GetWindowText(window,title,title.Capacity);
            var name=new StringBuilder(512); GetClassName(window,name,name.Capacity);
            windows.Add("title="+title+"; class="+name+"; visible="+IsWindowVisible(window)+"; responds="+Responds(window));
            return true;
        },IntPtr.Zero);
        return windows.ToArray();
    }
}
'@
$ProfilePath=(Resolve-Path -LiteralPath $ProfilePath).Path
$GameExe=(Resolve-Path -LiteralPath $GameExe).Path
$Me3Exe=(Get-Command $Me3Exe -ErrorAction Stop).Source
$processName=[IO.Path]::GetFileNameWithoutExtension($GameExe)
if (Get-Process -Name $processName -ErrorAction SilentlyContinue) {throw 'A game process is already running; no test started'}
$OutputRoot=[IO.Path]::GetFullPath($OutputRoot)
New-Item -ItemType Directory -Force -Path $OutputRoot | Out-Null
$tag=Get-Date -Format 'yyyyMMdd-HHmmss-fff'
$start=Get-Date
$clock=[Diagnostics.Stopwatch]::StartNew()
$launcher=Start-Process -FilePath $Me3Exe -ArgumentList @('launch','-p',('"'+$ProfilePath+'"'),'--exe',('"'+$GameExe+'"')) -WindowStyle Hidden -PassThru -RedirectStandardOutput (Join-Path $OutputRoot "$tag.stdout.log") -RedirectStandardError (Join-Path $OutputRoot "$tag.stderr.log")
$game=$null
$alive=$null
$gameWindow=[IntPtr]::Zero
$windowResponds=$false
$result='no_game'
try {
    while ($clock.Elapsed.TotalSeconds -lt $Seconds+20) {
        if (-not $game) {
            $game=Get-Process -Name $processName -ErrorAction SilentlyContinue | Where-Object { $_.StartTime -ge $start.AddSeconds(-1) -and $_.Path -eq $GameExe } | Select-Object -First 1
            if ($game) {$alive=[Diagnostics.Stopwatch]::StartNew()}
        } else {
            $game.Refresh()
            if ($game.HasExited) {$result='exited';break}
            if ($alive.Elapsed.TotalSeconds -ge $Seconds) {
                $gameWindow=[MinimapStartupWindows]::FindGameWindow($game.Id)
                $windowResponds=[MinimapStartupWindows]::Responds($gameWindow)
                $result=if ($windowResponds) {'responsive'} else {'unresponsive'}
                break
            }
        }
        if (-not $game -and $launcher.HasExited -and $launcher.ExitCode -ne 0) {$result='launcher_failed';break}
        Start-Sleep -Milliseconds 250
    }
    $state=[ordered]@{profile=$ProfilePath;result=$result;started_utc=$start.ToUniversalTime().ToString('o');elapsed_seconds=[Math]::Round($clock.Elapsed.TotalSeconds,2)}
    if ($game) {
        $game.Refresh()
        $state.game_pid=$game.Id
        $state.game_elapsed_seconds=[Math]::Round($alive.Elapsed.TotalSeconds,2)
        if (-not $game.HasExited) {
            $state.game_window_found=($gameWindow -ne [IntPtr]::Zero)
            $state.game_window_responding=$windowResponds
            $state.windows=[MinimapStartupWindows]::Describe($game.Id)
        }
    }
    $state | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $OutputRoot "$tag.result.json") -Encoding utf8
    $state | ConvertTo-Json
} finally {
    if ($game) {
        $game.Refresh()
        if (-not $game.HasExited) {
            $null=$game.CloseMainWindow()
            if (-not $game.WaitForExit(4000)) {$game.Kill()}
        }
    }
    if (-not $launcher.HasExited) {$launcher.Kill()}
}
if ($result -ne 'responsive') {exit 1}
