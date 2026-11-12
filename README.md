# litelaunch-zapret

Config picker for Flowseal's [zapret-discord-youtube](https://github.com/Flowseal/zapret-discord-youtube),
a built-in plugin of [LiteLaunch](https://github.com/LiteLaunch/litelaunch) (`z <name>`). It lives in the
launcher as `plugins/zapret` and is native (no Python).

## Where zapret comes from

By default the plugin keeps its own copy in `%APPDATA%\LiteLaunch\zapret\zapret-discord-youtube-<version>`,
so it works the same on any machine. On the first `z ` it offers «Установить zapret»: the latest release
from GitHub. If another zapret is found (a running winws.exe, or a `zapret*` folder in `C:\Progs`, `C:\`,
`D:\`, Desktop, Downloads, Documents or home), its settings are carried over.

«Выбрать папку zapret…» (at the end of the list) switches to a folder of your choice. It can be zapret
itself (updated in place) or a folder of versions (the newest is used, an update goes next to it and
replaces it). An empty folder also works: zapret gets installed into it. «Использовать встроенный zapret»
switches back.

## Rows

- **Configs.** Every `*.bat` except `service*.bat`, in natural order (ALT2 before ALT10). The plain `z `
  shows only the favorites (★) and the running config; «Конфиги» at the very bottom switches to
  `z конфиги`, the full list, favorites first, filtered by whatever follows. Typing a name right after
  `z ` searches all configs too. «Выбрать папку zapret…» is left out of the plain view (`z папка`).
  - Enter runs one elevated `cmd` (one UAC prompt): `net stop zapret`, kill `winws.exe`, then the config.
    It doesn't call the bat itself but a copy, `%APPDATA%\LiteLaunch\zapret\run.bat`, in which the winws.exe
    line uses relative paths (`..\lists\x` instead of `%LISTS%x`). With a long folder path such as
    `%APPDATA%`, the biggest strategies go over cmd's 8191-character limit (ALT11: 8482), cmd cuts the
    line and winws.exe never starts. If even the copy is too long, the row says so.
  - `NoDefaultCurrentDirectoryInExePath` is cleared first: some shells set it, and with it the bats
    can't find `service.bat`.
  - Right arrow: «Добавить в избранное» / «Убрать из избранного», «Показать в папке».
- **«Остановить zapret»**, while winws.exe runs. The running config is read from its console window
  title (`zapret: general (ALT)`).
- **«Обновить zapret: 1.10.3 → 1.10.4»**, when GitHub has a newer version. The check uses `curl.exe`
  in the background against the same `version.txt` that service.bat reads, every 6 h. Type `z обновить`
  to see the installed version or run the check by hand.
- **«Настройки zapret»** opens `service.bat admin`: zapret's own menu (service, Game Filter, ipset,
  diagnostics).
- **«Открыть папку zapret»**.

## Update (`update.ps1`)

The update runs elevated in a console window. It downloads the release .zip, stops zapret (the service,
winws.exe, the WinDivert driver) and copies the new version in. It keeps these settings:

- `lists\*-user.txt`;
- `utils\game_filter.enabled`;
- the update-check flag;
- the active fakes (`bin\ACTIVE_*.bin`);
- `utils\test results`;
- the ipset mode. Releases ship in "none" mode with the real list in `.backup`; the script puts the
  fresh list where the current mode expects it.

After that it starts again whatever was running: the service, or the same bat. If zapret is installed
as a service, its path is moved to the new folder. A new strategy only reaches the service after
reinstalling it from the settings.

The script is saved with a UTF-8 BOM: Windows PowerShell 5.1 reads BOM-less UTF-8 as ANSI.

## Config

`%APPDATA%\LiteLaunch\config.json`, section `zapret`:

| key | meaning |
|---|---|
| `keyword` | `z`; empty turns the plugin off |
| `dir` | empty: the plugin's own copy; otherwise your folder (see above) |
| `favorites` | config names, in display order |
| `last` | the config last started from the launcher |

`make_icon.py` generates all icons. `icon.png` is the plugin's own icon (the keyword hint);
`zapret.ico` is kept for a future notification icon. `icons/` holds one icon per row kind, unpacked
as `icon-<kind>.png`:

| kind | row |
|---|---|
| running (green shield) | the running config |
| favorite (amber star) | a favorite config |
| config (blue play) | any other config |
| warning (red !) | a config that won't start |
| stop (red square) | «Остановить zapret» |
| update (blue arrow) | install / update |
| settings (gear) | «Настройки zapret» |
| folder (orange) | «Открыть папку zapret» |
| folder-pick (teal) | choosing the folder, back to the built-in one |
| list (violet) | «Конфиги» |
