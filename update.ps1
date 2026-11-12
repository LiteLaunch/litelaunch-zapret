# Installs or updates Flowseal's zapret-discord-youtube from the latest GitHub release, keeping
# the user's settings. Started elevated by LiteLaunch (zapret.rs) in a console window.
#   -Base     where zapret lives: a folder of versions, or zapret itself (then updated in place)
#   -Root     the installed zapret to update (omitted for a fresh install)
#   -Source   another zapret to take settings from on a fresh install
#   -Restart  config (bat name) to start again if zapret was running
# Saved with a BOM: Windows PowerShell 5.1 reads BOM-less UTF-8 as ANSI.
param([Parameter(Mandatory)][string]$Base, [string]$Root, [string]$Source, [string]$Restart)

$ErrorActionPreference = 'Stop'
# zapret's bats call service.bat from their folder; this variable stops cmd from looking there.
$env:NoDefaultCurrentDirectoryInExePath = $null
$ProgressPreference = 'SilentlyContinue'
[Console]::OutputEncoding = [Text.Encoding]::UTF8
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
$Host.UI.RawUI.WindowTitle = 'LiteLaunch: обновление zapret'

function Version($dir) {
    $m = Select-String -LiteralPath (Join-Path $dir 'service.bat') -Pattern 'LOCAL_VERSION=([0-9.]+)' -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($m) { $m.Matches[0].Groups[1].Value } else { '' }
}

function Is-RealIpset($file) {
    if (-not (Test-Path -LiteralPath $file)) { return $false }
    $text = Get-Content -LiteralPath $file -Raw
    -not ([string]::IsNullOrWhiteSpace($text) -or $text -match '203\.0\.113\.113/32')
}

# User state that service.bat keeps inside the zapret folder.
function Keep-Settings($old, $new) {
    $files = 'lists\ipset-exclude-user.txt', 'lists\list-exclude-user.txt', 'lists\list-general-user.txt',
             'utils\game_filter.enabled', 'bin\ACTIVE_DISCORD_UDP.bin', 'bin\ACTIVE_GAME_UDP.bin'
    foreach ($f in $files) {
        if (Test-Path -LiteralPath "$old\$f") { Copy-Item -LiteralPath "$old\$f" -Destination "$new\$f" -Force }
    }
    # The update check at every bat start: on or off as it was.
    $flag = 'utils\check_updates.enabled'
    if (Test-Path -LiteralPath "$old\$flag") { Copy-Item -LiteralPath "$old\$flag" -Destination "$new\$flag" -Force }
    elseif (Test-Path -LiteralPath "$new\$flag") { Remove-Item -LiteralPath "$new\$flag" -Force }
    # ipset mode stays as it was. "loaded" = ipset-all.txt is the real list; "none" (placeholder)
    # and "any" (empty) keep the real list in ipset-all.txt.backup, where service.bat's switch
    # restores it from. Releases ship in "none" mode; the real list is the release's either way.
    $ipset = "$old\lists\ipset-all.txt"
    $all = "$new\lists\ipset-all.txt"
    $bak = "$all.backup"
    if (Test-Path -LiteralPath $ipset) {
        $oldLoaded = Is-RealIpset $ipset
        $newLoaded = Is-RealIpset $all
        if ($oldLoaded -and -not $newLoaded -and (Test-Path -LiteralPath $bak)) {
            Move-Item -LiteralPath $bak -Destination $all -Force
        } elseif (-not $oldLoaded) {
            if ($newLoaded) { Move-Item -LiteralPath $all -Destination $bak -Force }
            Copy-Item -LiteralPath $ipset -Destination $all -Force
        }
    }
    if (Test-Path -LiteralPath "$old\utils\test results") {
        Copy-Item -LiteralPath "$old\utils\test results" -Destination "$new\utils" -Recurse -Force
    }
}

$tmp = $null
try {
    $Base = $Base.TrimEnd('\')
    if ($Root) { $Root = $Root.TrimEnd('\') }
    $local = if ($Root) { Version $Root } else { '' }

    Write-Host 'Проверяю последнюю версию zapret на GitHub...'
    $rel = Invoke-RestMethod 'https://api.github.com/repos/Flowseal/zapret-discord-youtube/releases/latest' -Headers @{ 'User-Agent' = 'LiteLaunch' }
    $ver = $rel.tag_name
    if ($local -and $local -eq $ver) {
        Write-Host "Уже установлена последняя версия: $ver" -ForegroundColor Green
        Start-Sleep -Seconds 4
        exit 0
    }
    $asset = $rel.assets | Where-Object { $_.name -like '*.zip' } | Select-Object -First 1
    if (-not $asset) { throw "в релизе $ver нет .zip" }

    if ($local) { Write-Host "Обновление: $local -> $ver" } else { Write-Host "Установка zapret $ver в $Base" }
    $tmp = Join-Path $env:TEMP "litelaunch-zapret-$ver"
    if (Test-Path -LiteralPath $tmp) { Remove-Item -LiteralPath $tmp -Recurse -Force }
    New-Item -ItemType Directory -Path $tmp | Out-Null
    $zip = Join-Path $tmp $asset.name
    Write-Host "Скачиваю $($asset.name) ($([math]::Round($asset.size / 1MB, 1)) МБ)..."
    Invoke-WebRequest $asset.browser_download_url -OutFile $zip -UseBasicParsing
    Expand-Archive -LiteralPath $zip -DestinationPath "$tmp\x" -Force
    $new = Get-ChildItem -LiteralPath "$tmp\x" -Recurse -Filter 'service.bat' | Select-Object -First 1 | ForEach-Object { $_.DirectoryName }
    if (-not $new) { throw 'в архиве нет service.bat' }
    Get-ChildItem -LiteralPath $new -Recurse -File | Unblock-File

    $from = if ($Root) { $Root } else { $Source }
    if ($from -and (Test-Path -LiteralPath $from)) {
        Write-Host "Переношу настройки из $from"
        Keep-Settings $from $new
    }

    # Stop zapret: winws.exe holds the files, the WinDivert driver holds WinDivert64.sys.
    $svc = Get-Service -Name 'zapret' -ErrorAction SilentlyContinue
    $svcRunning = $svc -and $svc.Status -eq 'Running'
    $wasRunning = [bool](Get-Process -Name 'winws' -ErrorAction SilentlyContinue)
    if ($svcRunning -or $wasRunning) { Write-Host 'Останавливаю zapret...' }
    if ($svcRunning) { Stop-Service -Name 'zapret' -Force -ErrorAction SilentlyContinue }
    Get-Process -Name 'winws' -ErrorAction SilentlyContinue | Stop-Process -Force
    Start-Sleep -Milliseconds 500
    & sc.exe stop WinDivert *> $null
    Start-Sleep -Milliseconds 500

    # In place when Base is zapret itself, else a new version folder next to the old one.
    $inPlace = $Root -and ($Root -ieq $Base)
    $dest = if ($inPlace) { $Root } else { Join-Path $Base (Split-Path $new -Leaf) }
    New-Item -ItemType Directory -Path $dest -Force | Out-Null
    & robocopy.exe $new $dest /MIR /R:2 /W:1 /NFL /NDL /NJH /NJS /NP | Out-Null
    if ($LASTEXITCODE -ge 8) { throw "robocopy не смог скопировать файлы (код $LASTEXITCODE)" }
    if ($Root -and -not $inPlace -and (Test-Path -LiteralPath $Root) -and ($Root -ine $dest)) {
        Remove-Item -LiteralPath $Root -Recurse -Force -ErrorAction SilentlyContinue
        if (Test-Path -LiteralPath $Root) { Write-Host "Старую папку удалить не удалось: $Root" -ForegroundColor Yellow }
    }

    # A service installed from the old folder: point it at the new one.
    if ($svc -and $Root -and -not $inPlace) {
        $key = 'HKLM:\SYSTEM\CurrentControlSet\Services\zapret'
        $path = (Get-ItemProperty -Path $key).ImagePath
        $fixed = [regex]::Replace($path, [regex]::Escape($Root), $dest.Replace('$', '$$'), 'IgnoreCase')
        if ($fixed -ne $path) { Set-ItemProperty -Path $key -Name ImagePath -Value $fixed }
        Write-Host 'Служба zapret перенаправлена на новую папку; новая стратегия попадёт в неё после переустановки службы (Настройки zapret).' -ForegroundColor Yellow
    }

    if ($svcRunning) {
        Start-Service -Name 'zapret'
        Write-Host 'Служба zapret снова запущена.'
    } elseif ($wasRunning -and $Restart -and (Test-Path -LiteralPath "$dest\$Restart.bat")) {
        # The same rewrite as zapret.rs `runner`: relative paths in the winws.exe line, which with a
        # long folder path would otherwise overflow cmd's 8191-character limit.
        $bat = "$dest\$Restart.bat"
        $text = Get-Content -LiteralPath $bat -Raw
        $at = $text.IndexOf('start "zapret')
        if ($at -ge 0) {
            $winws = '"%BIN%winws.exe"'
            $tail = $text.Substring($at).Replace($winws, '@@WINWS@@').Replace('%BIN%', '').Replace('%LISTS%', '..\lists\').Replace('@@WINWS@@', $winws)
            $text = ($text.Substring(0, $at) + $tail).Replace('%~n0', $Restart).Replace('%~dp0', "$dest\")
            $bat = Join-Path $PSScriptRoot 'run.bat'
            [IO.File]::WriteAllText($bat, $text, (New-Object Text.UTF8Encoding $false))
        }
        Start-Process -FilePath 'cmd.exe' -ArgumentList ('/s /c ""' + $bat + '""') -WorkingDirectory $dest -WindowStyle Hidden
        Write-Host "Снова запущен: $Restart"
    }
    Write-Host "Готово: zapret $ver в $dest" -ForegroundColor Green
    Start-Sleep -Seconds 5
} catch {
    Write-Host "Ошибка: $($_.Exception.Message)" -ForegroundColor Red
    Read-Host 'Enter — закрыть'
    exit 1
} finally {
    if ($tmp -and (Test-Path -LiteralPath $tmp)) { Remove-Item -LiteralPath $tmp -Recurse -Force -ErrorAction SilentlyContinue }
}
