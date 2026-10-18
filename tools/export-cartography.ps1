param(
    [Parameter(Mandatory)][string[]]$LayoutRoots,
    [Parameter(Mandatory)][string]$NavigationRoot,
    [Parameter(Mandatory)][string]$OutputRoot,
    [Parameter(Mandatory)][string]$GameRoot,
    [Parameter(Mandatory)][string]$SmithboxRoot,
    [string[]]$MapIDs=@(),
    [int]$TimeoutSeconds=900
)
$ErrorActionPreference='Stop'
$clock=[Diagnostics.Stopwatch]::StartNew()
foreach($name in 'Andre.Core','Andre.SoulsFormats','Andre.IO','Andre.Formats') {Add-Type -Path (Join-Path $SmithboxRoot "$name.dll")}
[Runtime.InteropServices.NativeLibrary]::Load((Join-Path $GameRoot 'oo2core_6_win64.dll')) | Out-Null
[SoulsFormats.Oodle].GetField('Oodle6Exists',[Reflection.BindingFlags]'NonPublic,Public,Static').SetValue($null,$true)
$refs=@((Join-Path $SmithboxRoot 'Andre.SoulsFormats.dll'))+@(Get-ChildItem -LiteralPath (Join-Path $PSHOME 'ref') -Filter '*.dll' -File | ForEach-Object FullName)
Add-Type -Path (Join-Path $PSScriptRoot 'CartographyExport.cs') -ReferencedAssemblies $refs
$layouts=@{}
foreach($root in $LayoutRoots) {foreach($path in Get-ChildItem -LiteralPath $root -Filter layout.json -File -Recurse) {$layouts[$path.Directory.Name]=$path.FullName}}
$archives=@{}
$records=0;$completed=0;$skipped=0;$failed=0
try {
    foreach($family in 'outdoor','dungeons') {
        $destination=Join-Path $OutputRoot $family
        New-Item -ItemType Directory -Force -Path $destination | Out-Null
        foreach($nav in Get-ChildItem -LiteralPath (Join-Path $NavigationRoot $family) -Filter '*.nav' -File | Sort-Object Name) {
            if($clock.Elapsed.TotalSeconds -gt $TimeoutSeconds) {throw 'Cartography export time limit reached'}
            $id=$nav.BaseName
            if($MapIDs.Count -and $id -notin $MapIDs) {continue}
            if(-not $layouts.ContainsKey($id)) {continue}
            $layout=Get-Content -LiteralPath $layouts[$id] -Raw | ConvertFrom-Json
            $block=[byte[]]@($id.Substring(1,2),$id.Substring(4,2),$id.Substring(7,2),$id.Substring(10,2))
            $builder=[CartographyExport]::new()
            foreach($part in $layout.parts | Where-Object type -eq MapPiece) {
                if(@($part.rotation | Where-Object {[Math]::Abs($_) -gt 0.00001}).Count -gt 1) {$skipped++;continue}
                $virtual="/map/$($id.Substring(0,3))/$id/${id}_$($part.model.Substring(1)).mapbnd.dcx"
                $hash=[Andre.Core.Util.BhdDictionary]::ComputeHash($virtual,[SoulsFormats.BHD5+Game]::EldenRing)
                $bytes=$null
                $names=if($block[0] -ge 40 -and $block[0] -ne 60) {@('DLC','Data3','Data2','Data1','Data0')} else {@('Data3','Data2','Data1','Data0','DLC')}
                foreach($name in $names) {
                    if(-not $archives.ContainsKey($name)) {$archives[$name]=[Andre.Formats.BinderArchive]::new((Join-Path $GameRoot "$name.bhd"),(Join-Path $GameRoot "$name.bdt"),[Andre.Core.Game]::ER)}
                    $bytes=$archives[$name].TryReadFileFromHash($hash)
                    if($null -ne $bytes) {break}
                }
                if($null -eq $bytes) {$skipped++;continue}
                $offsetX=if($block[0] -ge 60) {$block[1]*256.0} else {0.0}
                $offsetZ=if($block[0] -ge 60) {$block[2]*256.0} else {0.0}
                try {$builder.Add([byte[]]$bytes,[float[]]$part.position,[float[]]$part.rotation,[float[]]$part.scale,$offsetX,$offsetZ)}
                catch {$failed++;Write-Output "Geometry unavailable for $id/$($part.model): $($_.Exception.InnerException.Message)"}
            }
            $builder.Write((Join-Path $destination "$id.land"),$block)
            $completed++;$records+=$builder.Count
            if($completed%25 -eq 0 -or $MapIDs.Count) {Write-Output "Cartography maps=$completed triangles=$records skipped=$skipped failed=$failed elapsed=$([int]$clock.Elapsed.TotalSeconds)s"}
        }
    }
    Write-Output "Finished cartography maps=$completed triangles=$records skipped=$skipped failed=$failed"
} finally {foreach($archive in $archives.Values) {$archive.Dispose()}}
