//! zapret — picks a config of Flowseal's zapret-discord-youtube (`z <name>`). Every
//! `general*.bat` in its folder is a config; Enter stops the running winws.exe and starts the
//! chosen one elevated. Favorites go first; the folder and service.bat (zapret's own settings
//! menu) are a row away.
//!
//! zapret itself is kept by the plugin in %APPDATA%\LiteLaunch\zapret\zapret-discord-youtube-<ver>
//! (or in a folder the user picks) and is installed and updated from GitHub by update.ps1,
//! which keeps the user's settings. A LiteLaunch plugin (DLL), no Python.

use litelaunch_plugin::{After, Host, Info, Item, Plugin, Reply, export_plugin};
use litelaunch_plugin::Action;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};
use windows::Win32::Foundation::*;
use windows::Win32::System::Diagnostics::ToolHelp::*;
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{BOOL, HSTRING, w};

const FILES: &[(&str, &[u8])] = &[
    ("icon.png", include_bytes!("icon.png")),
    ("update.ps1", include_bytes!("update.ps1")),
    ("icon-running.png", include_bytes!("icons/running.png")),
    ("icon-config.png", include_bytes!("icons/config.png")),
    ("icon-favorite.png", include_bytes!("icons/favorite.png")),
    ("icon-stop.png", include_bytes!("icons/stop.png")),
    ("icon-update.png", include_bytes!("icons/update.png")),
    ("icon-settings.png", include_bytes!("icons/settings.png")),
    ("icon-folder.png", include_bytes!("icons/folder.png")),
    ("icon-folder-pick.png", include_bytes!("icons/folder-pick.png")),
    ("icon-list.png", include_bytes!("icons/list.png")),
    ("icon-warning.png", include_bytes!("icons/warning.png")),
];

/// Title prefix of the console window general*.bat starts winws.exe in: `zapret: general (ALT)`.
const WINDOW_PREFIX: &str = "zapret: ";

/// The latest release's version, the same file service.bat checks.
const VERSION_URL: &str = "https://raw.githubusercontent.com/Flowseal/zapret-discord-youtube/main/.service/version.txt";

#[derive(Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct ZapretConfig {
    pub keyword: String,
    /// Empty: the plugin's own copy in %APPDATA%\LiteLaunch\zapret. Otherwise the user's folder:
    /// zapret itself (the one with service.bat, updated in place) or a folder holding versions of
    /// it (`C:\Progs\Zapret` with `zapret-discord-youtube-1.10.3` inside: the newest is used,
    /// an update goes next to it and replaces it).
    pub dir: String,
    /// Config names (bat file names without .bat), shown first in this order.
    pub favorites: Vec<String>,
    /// The config started last from the launcher, shown as running when its window is not found.
    pub last: String,
}

impl Default for ZapretConfig {
    fn default() -> Self {
        Self { keyword: "z".into(), dir: String::new(), favorites: Vec::new(), last: String::new() }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub enum Op {
    /// Config name: stop what runs, start `<name>.bat` elevated.
    Start(String),
    Stop,
    /// service.bat: zapret's own menu (service install, game filter, ipset, updates, diagnostics).
    Settings,
    OpenFolder,
    ToggleFavorite(String),
    /// "Конфиги": switch the query to the full list.
    ShowAll,
    /// Download the latest release (fresh install or update), keeping the settings.
    Update,
    /// A folder dialog: use zapret from a folder of the user's choice.
    ChooseFolder,
    /// Back to the plugin's own copy.
    UseBuiltin,
}

/// The plugin's data folder, %APPDATA%\LiteLaunch\zapret (from the launcher, see `Plugin::init`).
static DATA_DIR: OnceLock<PathBuf> = OnceLock::new();

pub fn dir() -> PathBuf {
    DATA_DIR.get().cloned().unwrap_or_else(|| PathBuf::from(std::env::var("APPDATA").unwrap_or_default()).join("LiteLaunch").join("zapret"))
}

#[derive(Default)]
pub struct Zapret;

export_plugin!(Zapret);

impl Plugin for Zapret {
    type Settings = ZapretConfig;
    type Op = Op;

    fn info(&self, host: &Host) -> Info {
        let _ = DATA_DIR.set(host.data_dir.clone());
        Info {
            name: "Zapret".into(),
            description: "Запуск конфигов zapret-discord-youtube (Flowseal) в один Enter: избранное, остановка, установка и обновление с GitHub".into(),
            version: env!("CARGO_PKG_VERSION").into(),
            icon: Some(icon()),
            ..Default::default()
        }
    }

    fn init(&self, host: &Host) {
        let _ = DATA_DIR.set(host.data_dir.clone());
        install();
    }

    fn query(&self, _host: &Host, cfg: &ZapretConfig, q: &str) -> Reply {
        search(cfg, q).into()
    }

    fn actions(&self, _host: &Host, cfg: &ZapretConfig, item: &Item) -> Vec<Item> {
        actions(cfg, item)
    }

    fn run(&self, _host: &Host, cfg: &mut ZapretConfig, op: Op) -> After {
        run(&op, cfg)
    }
}

pub fn icon() -> String {
    dir().join("icon.png").to_string_lossy().into()
}

pub fn install() {
    litelaunch_plugin::util::unpack(&dir(), FILES);
}

/// Where zapret lives: the configured folder or the plugin's own one.
fn base(cfg: &ZapretConfig) -> PathBuf {
    match cfg.dir.trim() {
        "" => dir(),
        d => PathBuf::from(litelaunch_plugin::util::expand_env(d)),
    }
}

/// The folder with service.bat: the base itself or its newest zapret subfolder.
pub fn root(cfg: &ZapretConfig) -> Option<PathBuf> {
    let d = base(cfg);
    if is_zapret(&d) {
        return Some(d);
    }
    std::fs::read_dir(&d)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .filter(|p| is_zapret(p))
        .max_by(|a, b| natural_cmp(&name_of(a), &name_of(b)))
}

fn is_zapret(dir: &Path) -> bool {
    dir.join("service.bat").is_file() && dir.join("bin").join("winws.exe").is_file()
}

fn name_of(p: &Path) -> String {
    p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

/// `set "LOCAL_VERSION=1.10.3"` from service.bat.
fn local_version(root: &Path) -> Option<String> {
    let text = std::fs::read_to_string(root.join("service.bat")).ok()?;
    let rest = &text[text.find("LOCAL_VERSION=")? + 14..];
    Some(rest.chars().take_while(|c| c.is_ascii_digit() || *c == '.').collect()).filter(|v: &String| !v.is_empty())
}

/// The latest version, checked in the background with curl.exe (every 6 h; 10 min after a failure).
/// None until known.
fn latest_version() -> Option<String> {
    static LATEST: Mutex<Option<(Instant, Option<String>)>> = Mutex::new(None);
    let mut latest = LATEST.lock().unwrap();
    let stale = match &*latest {
        None => true,
        Some((at, v)) => at.elapsed() > if v.is_some() { Duration::from_secs(6 * 3600) } else { Duration::from_secs(600) },
    };
    if stale {
        let known = latest.as_ref().and_then(|(_, v)| v.clone());
        // Marks the check as running, so keystrokes meanwhile don't start more.
        *latest = Some((Instant::now(), known));
        std::thread::spawn(|| {
            use std::os::windows::process::CommandExt;
            let out = std::process::Command::new("curl.exe")
                .args(["-s", "-f", "-m", "8", VERSION_URL])
                .creation_flags(litelaunch_plugin::util::CREATE_NO_WINDOW)
                .output();
            let v = out.ok().filter(|o| o.status.success()).map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string());
            let v = v.filter(|v| !v.is_empty() && v.chars().all(|c| c.is_ascii_digit() || c == '.'));
            let mut latest = LATEST.lock().unwrap();
            let keep = latest.as_ref().and_then(|(_, old)| old.clone());
            *latest = Some((Instant::now(), v.or(keep)));
        });
    }
    latest.as_ref().and_then(|(_, v)| v.clone())
}

/// A zapret installed elsewhere, to take settings from on a fresh install: the running winws.exe
/// (`<zapret>\bin\winws.exe`), else a folder named zapret* up to two levels deep in usual places.
fn detect() -> Option<PathBuf> {
    if let Some(exe) = winws_path() {
        if let Some(z) = Path::new(&exe).parent().and_then(Path::parent).filter(|p| is_zapret(p)) {
            return Some(z.to_path_buf());
        }
    }
    let home = std::env::var("USERPROFILE").unwrap_or_default();
    let places = ["C:\\Progs".to_string(), "C:\\Program Files".into(), "C:\\".into(), "D:\\".into(), format!("{home}\\Desktop"), format!("{home}\\Downloads"), format!("{home}\\Documents"), home];
    for place in places {
        let Ok(entries) = std::fs::read_dir(&place) else { continue };
        for e in entries.flatten().map(|e| e.path()).filter(|p| p.is_dir()) {
            if !name_of(&e).to_lowercase().contains("zapret") {
                continue;
            }
            if is_zapret(&e) {
                return Some(e);
            }
            let Ok(rd) = std::fs::read_dir(&e) else { continue };
            let inner = rd.flatten().map(|x| x.path()).filter(|p| is_zapret(p));
            if let Some(z) = inner.max_by(|a, b| natural_cmp(&name_of(a), &name_of(b))) {
                return Some(z);
            }
        }
    }
    None
}

/// Config names in the folder: every .bat except service*.bat (as service.bat itself does).
fn configs(root: &Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(root)
        .map(|rd| {
            rd.flatten()
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("bat")))
                .filter_map(|p| p.file_stem().map(|s| s.to_string_lossy().into_owned()))
                .filter(|n| !n.to_lowercase().starts_with("service"))
                .collect()
        })
        .unwrap_or_default();
    v.sort_by(|a, b| natural_cmp(a, b));
    v
}

/// "ALT2" before "ALT10"; case-insensitive otherwise.
fn natural_cmp(a: &str, b: &str) -> Ordering {
    let (a, b) = (a.to_lowercase(), b.to_lowercase());
    let (mut x, mut y) = (a.chars().peekable(), b.chars().peekable());
    loop {
        match (x.peek().copied(), y.peek().copied()) {
            (None, None) => return Ordering::Equal,
            (None, _) => return Ordering::Less,
            (_, None) => return Ordering::Greater,
            (Some(c), Some(d)) if c.is_ascii_digit() && d.is_ascii_digit() => {
                let num = |it: &mut std::iter::Peekable<std::str::Chars>| {
                    let mut s = String::new();
                    while let Some(c) = it.peek().copied().filter(char::is_ascii_digit) {
                        s.push(c);
                        it.next();
                    }
                    s.parse::<u64>().unwrap_or(0)
                };
                let o = num(&mut x).cmp(&num(&mut y));
                if o != Ordering::Equal {
                    return o;
                }
            }
            (Some(c), Some(d)) => {
                if c != d {
                    return c.cmp(&d);
                }
                x.next();
                y.next();
            }
        }
    }
}

/// Full path of a running winws.exe, if any; Some("") when running but the path is hidden.
fn winws_path() -> Option<String> {
    unsafe {
        let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0).ok()?;
        let mut e = PROCESSENTRY32W { dwSize: size_of::<PROCESSENTRY32W>() as u32, ..Default::default() };
        let mut ok = Process32FirstW(snap, &mut e).is_ok();
        let mut found = None;
        while ok {
            let len = e.szExeFile.iter().position(|&c| c == 0).unwrap_or(e.szExeFile.len());
            if String::from_utf16_lossy(&e.szExeFile[..len]).eq_ignore_ascii_case("winws.exe") {
                found = Some(process_path(e.th32ProcessID).unwrap_or_default());
                break;
            }
            ok = Process32NextW(snap, &mut e).is_ok();
        }
        let _ = CloseHandle(snap);
        found
    }
}

fn process_path(pid: u32) -> Option<String> {
    use windows::Win32::System::Threading::*;
    unsafe {
        let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut buf = [0u16; 1024];
        let mut len = buf.len() as u32;
        let r = QueryFullProcessImageNameW(h, PROCESS_NAME_WIN32, windows::core::PWSTR(buf.as_mut_ptr()), &mut len);
        let _ = CloseHandle(h);
        r.ok().map(|_| String::from_utf16_lossy(&buf[..len as usize]))
    }
}

/// What runs now, if anything.
struct Running {
    /// The config (bat name), from its console window title or the last one started.
    config: Option<String>,
    /// For the stop row.
    note: String,
}

/// zapret is on if winws.exe runs, the zapret service runs, or the WinDivert driver is still
/// loaded (left behind by a winws.exe that was killed; stopping unloads it).
fn running(cfg: &ZapretConfig) -> Option<Running> {
    let winws = winws_path().is_some();
    let service = service_running(w!("zapret"));
    let divert = service_running(w!("WinDivert"));
    if service {
        return Some(Running { config: None, note: "Работает служба zapret".into() });
    }
    if !winws {
        return divert.then(|| Running { config: None, note: "winws.exe не запущен, но драйвер WinDivert загружен".into() });
    }
    unsafe extern "system" fn each(hwnd: HWND, out: LPARAM) -> BOOL {
        let mut buf = [0u16; 256];
        let n = unsafe { GetWindowTextW(hwnd, &mut buf) } as usize;
        let title = String::from_utf16_lossy(&buf[..n]);
        if let Some(name) = title.strip_prefix(WINDOW_PREFIX) {
            unsafe { *(out.0 as *mut String) = name.trim().to_string() };
            return false.into();
        }
        true.into()
    }
    let mut name = String::new();
    unsafe {
        let _ = EnumWindows(Some(each), LPARAM(&mut name as *mut String as isize));
    }
    let config = if !name.is_empty() { Some(name) } else { Some(cfg.last.clone()).filter(|l| !l.is_empty()) };
    let note = match &config {
        Some(c) => format!("Работает: {c}"),
        None => "winws.exe работает".into(),
    };
    Some(Running { config, note })
}

/// A Windows service exists and is not stopped. Querying status needs no admin rights.
fn service_running(name: windows::core::PCWSTR) -> bool {
    use windows::Win32::System::Services::*;
    unsafe {
        let Ok(scm) = OpenSCManagerW(None, None, SC_MANAGER_CONNECT) else { return false };
        let mut on = false;
        if let Ok(svc) = OpenServiceW(scm, name, SERVICE_QUERY_STATUS) {
            let mut st = SERVICE_STATUS::default();
            on = QueryServiceStatus(svc, &mut st).is_ok() && st.dwCurrentState != SERVICE_STOPPED;
            let _ = CloseServiceHandle(svc);
        }
        let _ = CloseServiceHandle(scm);
        on
    }
}

/// A row with the icon of its kind (see make_icon.py); configs pick theirs in `search`.
fn item(title: impl Into<String>, sub: impl Into<String>, score: i64, op: Option<Op>) -> Item {
    let kind = match &op {
        Some(Op::Stop) => "stop",
        Some(Op::Update) => "update",
        Some(Op::Settings) => "settings",
        Some(Op::OpenFolder) => "folder",
        Some(Op::ChooseFolder | Op::UseBuiltin) => "folder-pick",
        Some(Op::ShowAll) => "list",
        Some(_) => "config",
        None => "warning",
    };
    let action = op.as_ref().map_or(Action::Nothing, Action::op);
    let mut it = Item::new(title, sub, Some(icon_of(kind)), score, action);
    // The folder dialog would open under the launcher window.
    it.hide_first = matches!(op, Some(Op::ChooseFolder));
    it
}

/// The op behind one of our rows.
fn op_of(it: &Item) -> Option<Op> {
    match &it.action {
        Action::Plugin(v) => serde_json::from_value(v.clone()).ok(),
        _ => None,
    }
}

/// `icons/<kind>.png`, unpacked as `icon-<kind>.png`.
fn icon_of(kind: &str) -> String {
    dir().join(format!("icon-{kind}.png")).to_string_lossy().into()
}

/// Where zapret is taken from, for row subtitles.
fn place(cfg: &ZapretConfig) -> String {
    let b = base(cfg).to_string_lossy().into_owned();
    if cfg.dir.trim().is_empty() { format!("встроенный: {b}") } else { format!("своя папка: {b}") }
}

pub fn search(cfg: &ZapretConfig, q: &str) -> Vec<Item> {
    let wants = |words: &[&str]| q.is_empty() || words.iter().any(|w| litelaunch_plugin::fuzzy::score(w, q).is_some_and(|s| s >= 450));
    // Rarely needed: at the very end, or found by typing "папка"/"встроенный".
    let source_rows = |items: &mut Vec<Item>| {
        if wants(&["выбрать папку", "папка", "folder"]) {
            items.push(item("Выбрать папку zapret…", format!("Сейчас {}", place(cfg)), 0, Some(Op::ChooseFolder)));
        }
        if !cfg.dir.trim().is_empty() && wants(&["встроенный", "builtin", "папка"]) {
            let sub = format!("Свой zapret плагина в {}", dir().to_string_lossy());
            items.push(item("Использовать встроенный zapret", sub, 0, Some(Op::UseBuiltin)));
        }
    };

    let Some(root) = root(cfg) else {
        let target = base(cfg).to_string_lossy().into_owned();
        let sub = match detect() {
            Some(other) => format!("Последняя версия с GitHub в {target}; настройки возьмутся из {}", other.to_string_lossy()),
            None => format!("Последняя версия с GitHub в {target}"),
        };
        let mut items = vec![item("Установить zapret", sub, 0, Some(Op::Update))];
        source_rows(&mut items);
        return items;
    };

    // `z конфиги [filter]`: every config and nothing else.
    let ql = q.to_lowercase();
    let all = (ql == ALL_WORD || ql.starts_with(&format!("{ALL_WORD} "))).then(|| q[ALL_WORD.len()..].trim());
    let filter = all.unwrap_or(q);

    let running = running(cfg);
    let configs = configs(&root);
    let mut items = Vec::new();
    for name in &configs {
        let name = name.clone();
        let fav = cfg.favorites.iter().position(|f| f.eq_ignore_ascii_case(&name));
        let is_running = running.as_ref().and_then(|r| r.config.as_deref()).is_some_and(|r| r.eq_ignore_ascii_case(&name));
        let score = if filter.is_empty() {
            // The plain `z `: only favorites (in their order) and the running config; the rest
            // sit behind "Конфиги". The full list: favorites first, then folder order (stable sort).
            if all.is_none() && fav.is_none() && !is_running {
                continue;
            }
            fav.map_or(if is_running { 1 } else { 0 }, |i| 10_000 - i as i64)
        } else {
            let Some(s) = litelaunch_plugin::fuzzy::score(&name, filter) else { continue };
            s as i64 + if fav.is_some() { 200 } else { 0 }
        };
        let title = if fav.is_some() { format!("★ {name}") } else { name.clone() };
        let sub = match (is_running, fav.is_some()) {
            (true, _) => "Работает сейчас · Enter — перезапустить".to_string(),
            (false, true) => "Избранное · Enter — запустить".to_string(),
            (false, false) => "Enter — запустить · → в избранное".to_string(),
        };
        let too_long = runner(&root, &name).map(|r| command_len(&r, &root)).filter(|&n| n > CMD_LIMIT);
        let sub = match too_long {
            Some(n) => format!("⚠ Не запустится: команда {n} символов, предел cmd {CMD_LIMIT}. Сообщите автору плагина"),
            None => sub,
        };
        let mut it = item(title, sub, score, Some(Op::Start(name.clone())));
        let kind = match (is_running, fav.is_some(), too_long.is_some()) {
            (_, _, true) => "warning",
            (true, _, _) => "running",
            (false, true, _) => "favorite",
            _ => "config",
        };
        it.icon = Some(icon_of(kind));
        it.file = Some(root.join(format!("{name}.bat")).to_string_lossy().into_owned());
        items.push(it);
    }
    items.sort_by(|a, b| b.score.cmp(&a.score));
    if all.is_some() {
        if items.is_empty() {
            items.push(item("Нет такого конфига", format!("Конфиги — general*.bat в {}", root.to_string_lossy()), 0, None));
        }
        return items;
    }

    // On: first row. Off: still there when asked for ("z стоп"), in case it runs unseen.
    let stop_words: &[&str] = &["остановить", "стоп", "выключить", "отключить", "stop"];
    match &running {
        Some(r) if wants(stop_words) => items.insert(0, item("Остановить zapret", r.note.clone(), 0, Some(Op::Stop))),
        None if !q.is_empty() && wants(stop_words) => {
            items.insert(0, item("Остановить zapret", "Похоже, не запущен; Enter всё равно остановит winws.exe, службу и WinDivert", 0, Some(Op::Stop)))
        }
        _ => {}
    }
    // With no query they go right after the favorites, not below two dozen configs.
    let mut at = if q.is_empty() {
        items.iter().take_while(|it| it.score > 0 || matches!(op_of(it), Some(Op::Stop))).count()
    } else {
        items.len()
    };
    let mut put = |items: &mut Vec<Item>, it: Item| {
        items.insert(at, it);
        at += 1;
    };
    let local = local_version(&root).unwrap_or_default();
    let latest = latest_version();
    let newer = latest.as_ref().filter(|l| natural_cmp(l, &local) == Ordering::Greater);
    if let Some(l) = newer {
        if wants(&["обновить", "update"]) {
            put(&mut items, item(format!("Обновить zapret: {local} → {l}"), "Скачать с GitHub; списки и настройки сохранятся", 0, Some(Op::Update)));
        }
    }
    if wants(&["настройки", "settings", "service"]) {
        put(&mut items, item("Настройки zapret", "service.bat: служба, Game Filter, ipset, обновления, диагностика", 0, Some(Op::Settings)));
    }
    if wants(&["папка", "folder", "открыть"]) {
        put(&mut items, item("Открыть папку zapret", root.to_string_lossy().into_owned(), 0, Some(Op::OpenFolder)));
    }
    // No newer version: the update row only when asked for, saying what is installed.
    if newer.is_none() && !q.is_empty() && wants(&["обновить", "update"]) {
        let sub = match &latest {
            Some(l) => format!("Установлена {local}, последняя {l} · Enter — проверить ещё раз"),
            None => format!("Установлена {local} · Enter — проверить на GitHub"),
        };
        items.push(item("Обновить zapret", sub, 0, Some(Op::Update)));
    }
    // Not in the plain `z ` (it would push "Конфиги" below the fold): `z папка` finds them.
    if !q.is_empty() {
        source_rows(&mut items);
    }
    if q.is_empty() {
        let sub = format!("Все {} · Enter — показать, дальше можно печатать название", configs.len());
        items.push(item("Конфиги", sub, 0, Some(Op::ShowAll)));
    }
    if items.is_empty() {
        items.push(item("Нет такого конфига", format!("Конфиги — general*.bat в {}", root.to_string_lossy()), 0, None));
    }
    items
}

/// Extra rows for a config's actions menu (Right arrow).
pub fn actions(cfg: &ZapretConfig, item: &Item) -> Vec<Item> {
    let Some(Op::Start(name)) = op_of(item) else { return Vec::new() };
    let fav = cfg.favorites.iter().any(|f| f.eq_ignore_ascii_case(&name));
    let (glyph, title) = if fav { ('\u{E8D9}', "Убрать из избранного") } else { ('\u{E734}', "Добавить в избранное") };
    let mut it = Item::new(title, name.clone(), None, 0, Action::op(&Op::ToggleFavorite(name.clone())));
    it.glyph = Some(glyph);
    vec![it]
}

/// `z конфиги`: the full list of configs.
const ALL_WORD: &str = "конфиги";

/// Runs an op; the launcher saves `cfg` if it changed.
pub fn run(op: &Op, cfg: &mut ZapretConfig) -> After {
    match op {
        Op::Update => {
            update(cfg);
            return After::Hide;
        }
        Op::ChooseFolder => {
            let Some(picked) = pick_folder(&base(cfg)) else { return After::Hide };
            // A versioned zapret folder: take the folder of versions, so updates go next to it.
            let use_dir = match picked.parent() {
                Some(parent) if is_zapret(&picked) && name_of(&picked).to_lowercase().starts_with("zapret-discord-youtube-") => parent.to_path_buf(),
                _ => picked,
            };
            cfg.dir = if use_dir == dir() { String::new() } else { use_dir.to_string_lossy().into_owned() };
            return After::Hide;
        }
        Op::UseBuiltin => {
            cfg.dir.clear();
            return After::Requery;
        }
        Op::ShowAll => return After::SetQuery(format!("{} {ALL_WORD} ", cfg.keyword)),
        _ => {}
    }
    let Some(root) = root(cfg) else { return After::Hide };
    // Stops a service-installed zapret too: it would fight the new winws.exe for WinDivert.
    // WinDivert goes too: a killed winws.exe leaves the driver loaded.
    const STOP: &str = "net stop zapret >nul 2>&1 & taskkill /F /IM winws.exe >nul 2>&1 & net stop WinDivert >nul 2>&1";
    match op {
        Op::Start(name) => {
            // The rewritten copy; the bat itself if it isn't shaped like Flowseal's.
            let run = dir().join("run.bat");
            let run = match runner(&root, name) {
                Some(text) if std::fs::write(&run, &text).is_ok() => run,
                _ => root.join(format!("{name}.bat")),
            };
            elevated_cmd(&format!("{STOP} & call \"{}\"", run.to_string_lossy()), &root, SW_HIDE);
            cfg.last = name.clone();
            After::Hide
        }
        Op::Stop => {
            elevated_cmd(STOP, &root, SW_HIDE);
            After::Hide
        }
        Op::Settings => {
            // The "admin" argument skips service.bat's own (second) elevation request.
            let bat = root.join("service.bat");
            elevated_cmd(&format!("call \"{}\" admin", bat.to_string_lossy()), &root, SW_SHOWNORMAL);
            After::Hide
        }
        Op::OpenFolder => {
            let _ = std::process::Command::new("explorer.exe").arg(&root).spawn();
            After::Hide
        }
        Op::ToggleFavorite(name) => {
            match cfg.favorites.iter().position(|f| f.eq_ignore_ascii_case(name)) {
                Some(i) => {
                    cfg.favorites.remove(i);
                }
                None => cfg.favorites.push(name.clone()),
            }
            After::Requery
        }
        Op::Update | Op::ChooseFolder | Op::UseBuiltin | Op::ShowAll => After::Hide,
    }
}

/// update.ps1, elevated, in its own console window: installs or updates zapret in the base
/// folder, then starts again what was running.
fn update(cfg: &ZapretConfig) {
    install();
    // Paths go in quotes; a trailing backslash would escape the closing quote.
    let arg = |name: &str, value: &Path| format!(" -{name} \"{}\"", value.to_string_lossy().trim_end_matches('\\'));
    let script = dir().join("update.ps1");
    let mut params = format!("-NoProfile -ExecutionPolicy Bypass -File \"{}\"", script.to_string_lossy());
    params += &arg("Base", &base(cfg));
    match root(cfg) {
        Some(root) => {
            params += &arg("Root", &root);
            if let Some(name) = running(cfg).and_then(|r| r.config).filter(|n| root.join(format!("{n}.bat")).is_file()) {
                params += &format!(" -Restart \"{name}\"");
            }
        }
        None => {
            if let Some(other) = detect() {
                params += &arg("Source", &other);
            }
        }
    }
    unsafe {
        ShellExecuteW(None, w!("runas"), w!("powershell.exe"), &HSTRING::from(params), None, SW_SHOWNORMAL);
    }
}

/// The system folder dialog; None if cancelled.
fn pick_folder(start: &Path) -> Option<PathBuf> {
    use windows::Win32::System::Com::*;
    use windows::Win32::UI::Shell::*;
    unsafe {
        let dlg: IFileOpenDialog = CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER).ok()?;
        dlg.SetOptions(dlg.GetOptions().ok()? | FOS_PICKFOLDERS | FOS_FORCEFILESYSTEM).ok()?;
        let _ = dlg.SetTitle(w!("Папка zapret: сам zapret, папка с его версиями или пустая для установки"));
        if let Ok(folder) = SHCreateItemFromParsingName::<_, _, IShellItem>(&HSTRING::from(start.as_os_str()), None) {
            let _ = dlg.SetFolder(&folder);
        }
        dlg.Show(None).ok()?;
        let path = dlg.GetResult().ok()?.GetDisplayName(SIGDN_FILESYSPATH).ok()?;
        let s = path.to_string().ok();
        CoTaskMemFree(Some(path.0 as _));
        s.map(PathBuf::from)
    }
}

/// cmd's command line limit. The bats pass winws.exe a few dozen paths inside the zapret folder,
/// so with a long folder path the biggest strategies go over it (ALT11 in %APPDATA%: 8482), cmd
/// cuts the line and winws.exe never starts. Hence `runner`.
const CMD_LIMIT: usize = 8191;

/// A config's bat rewritten to run from anywhere with a short winws.exe line: `%~dp0` and `%~n0`
/// become the folder and the config name, and inside the winws.exe line `%BIN%x` / `%LISTS%x`
/// become `x` / `..\lists\x` (the bat runs winws.exe from bin, so relative paths resolve there).
/// The rest of the bat (status, update check, game filter, user lists) is left as it is.
fn runner(root: &Path, name: &str) -> Option<String> {
    let text = std::fs::read_to_string(root.join(format!("{name}.bat"))).ok()?;
    let at = text.find("start \"zapret")?;
    let (head, tail) = text.split_at(at);
    const WINWS: &str = "\"%BIN%winws.exe\"";
    let tail = tail.replace(WINWS, "\u{1}").replace("%BIN%", "").replace("%LISTS%", "..\\lists\\").replace('\u{1}', WINWS);
    let folder = format!("{}\\", root.to_string_lossy().trim_end_matches('\\'));
    Some(format!("{head}{tail}").replace("%~n0", name).replace("%~dp0", &folder))
}

/// Length of the winws.exe line the runner starts, expanded (the game filter variables counted
/// at their shortest).
fn command_len(runner: &str, root: &Path) -> usize {
    let Some(at) = runner.find("start \"zapret") else { return 0 };
    let line = runner[at..].replace("^\r\n", "").replace("^\n", "");
    let line = line.lines().next().unwrap_or_default();
    let bin = format!("{}\\bin\\", root.to_string_lossy().trim_end_matches('\\'));
    line.replace("%BIN%", &bin).replace("%GameFilterTCP%", "12").replace("%GameFilterUDP%", "12").replace("%GameFilter%", "12").trim().len()
}

/// `cmd /s /c "<line>"` as administrator (one UAC prompt), in `dir`.
fn elevated_cmd(line: &str, dir: &Path, show: SHOW_WINDOW_CMD) {
    // zapret's bats run `call service.bat` from their folder. With NoDefaultCurrentDirectoryInExePath
    // set (inherited from some shells), cmd doesn't look there, and winws.exe never starts.
    let params = HSTRING::from(format!("/s /c \"set \"NoDefaultCurrentDirectoryInExePath=\" & {line}\""));
    unsafe {
        ShellExecuteW(None, w!("runas"), w!("cmd.exe"), &params, &HSTRING::from(dir.as_os_str()), show);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn natural_order() {
        let mut v = vec!["general (ALT10)", "general (ALT2)", "general", "general (ALT)", "general (EXP)", "general (ALT9)"];
        v.sort_by(|a, b| natural_cmp(a, b));
        assert_eq!(v, ["general", "general (ALT)", "general (ALT2)", "general (ALT9)", "general (ALT10)", "general (EXP)"]);
        assert_eq!(natural_cmp("zapret-discord-youtube-1.10.3", "zapret-discord-youtube-1.9.9"), Ordering::Greater);
    }
}
