// PartyBox Shell: a window that is nothing but a webview pointed at one URL, and
// that can take over the whole screen ("kiosk mode") with no bars of any kind.
//
//   partybox-shell [options]        (every option also works as --name=value)
//
// See USAGE below (or --help). In short: the window opens where it was closed
// (same monitor, same mode); the very first time, or if that screen is gone, it
// opens as a normal window so it can be dragged to the right screen first.
//
// In debug builds there's a native menu with a shortcut to enter/leave kiosk
// mode (Cmd/Ctrl+Shift+K), reload and quit, so it can be worked on comfortably.
// Release builds have no menu and no way out of kiosk mode: that's the point
// (start them with --windowed to get a normal window again).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::net::{TcpStream, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use tauri::menu::{MenuBuilder, MenuItemBuilder, SubmenuBuilder};
use tauri::{AppHandle, Manager, PhysicalPosition, PhysicalSize, State, Url, WebviewUrl, WebviewWindow, WebviewWindowBuilder, WindowEvent};

const DEFAULT_URL: &str = "http://localhost:5173";
const WINDOW: &str = "main";

/// Runs at the start of every page the webview loads, the remote site included.
const KIOSK_SCRIPT: &str = r#"
  // A public touch screen has no use for a context menu, pinch/scroll zoom or
  // text-selection drags: none of them are wanted, all of them look broken.
  document.addEventListener('contextmenu', (e) => e.preventDefault());
  document.addEventListener('gesturestart', (e) => e.preventDefault());
  window.addEventListener('wheel', (e) => { if (e.ctrlKey) e.preventDefault(); }, { passive: false });
"#;

/// With `--dev-tools`: sets the flag the PWA's own `config/devTools.js`
/// checks (alongside `import.meta.env.DEV` and `VITE_DEV_TOOLS`), so its dev
/// panels (local switcher, camera settings...) show up without a separate
/// build or touching the deployed site at all — a choice made fresh each
/// time the shell starts, not baked into any deploy. Runs before every page
/// load (the local splash included), so it's already set by the time the
/// real site's own scripts run, even across the splash -> target navigation.
const DEV_TOOLS_SCRIPT: &str = "window.__PARTYBOX_DEV_TOOLS__ = true;";

const USAGE: &str = "\
partybox-shell [options]        (each option also works as --name=value)

  --url <url>          the site to show (or env PARTYBOX_URL). Default http://localhost:5173
  --rotate=<deg>       turn the whole interface: 0, 90, 180 or 270 (clockwise). For a touch
                       panel mounted turned; it becomes ?rotate=<deg> on the URL
  --cursor=<hide|show> hide or show the mouse pointer (?cursor=... on the URL, and the native
                       pointer too in kiosk mode). Without it the site decides: hidden in
                       production builds, shown in development
  --hide-cursor        same as --cursor=hide
  --kiosk              start in kiosk mode (fullscreen, no bars, no way out with the mouse)
  --windowed           start as a normal window
  --reset-window       forget the remembered screen and window position
  --dev-tools          turn on the PWA's own dev panels (local switcher, camera settings...)
                       even on a deployed build, without touching its own env vars or
                       redeploying — see src/config/devTools.js on the PartyBox side
  -h, --help           this text

--rotate and --cursor override the same parameters if the URL already has them.
Without --kiosk or --windowed the shell starts the way it was closed, on the same
screen; if there's nothing remembered (or that screen isn't connected any more) it
starts as a normal window, so it can be moved to the right screen first.";

#[derive(Debug, Clone, Copy, PartialEq)]
enum Cursor {
    Hide,
    Show,
}

impl Cursor {
    fn as_str(self) -> &'static str {
        match self {
            Cursor::Hide => "hide",
            Cursor::Show => "show",
        }
    }
}

#[derive(Debug)]
struct Args {
    /// Already carries ?rotate= / ?cursor= when those options were given.
    url: Url,
    /// `--kiosk` / `--windowed`; `None` = however it was left.
    kiosk: Option<bool>,
    hide_cursor: bool,
    reset_window: bool,
    dev_tools: bool,
}

/// What the command line asked for; `Help` means "print USAGE and stop".
enum Parsed {
    Run(Args),
    Help,
}

fn no_value(name: &str, inline: &Option<String>) -> Result<(), String> {
    match inline {
        Some(_) => Err(format!("{name} takes no value")),
        None => Ok(()),
    }
}

/// `--name value` and `--name=value` both work.
fn parse_args(argv: impl IntoIterator<Item = String>, env_url: Option<String>) -> Result<Parsed, String> {
    let mut url = env_url.unwrap_or_else(|| DEFAULT_URL.into());
    let mut kiosk = None;
    let mut cursor = None;
    let mut rotate = None;
    let mut reset_window = false;
    let mut dev_tools = false;

    let mut argv = argv.into_iter();
    while let Some(arg) = argv.next() {
        let (key, inline) = match arg.split_once('=') {
            Some((k, v)) if k.starts_with("--") => (k.to_owned(), Some(v.to_owned())),
            _ => (arg, None),
        };
        match key.as_str() {
            "-h" | "--help" => return Ok(Parsed::Help),
            "--url" => url = inline.or_else(|| argv.next()).ok_or("--url needs a value")?,
            "--rotate" => {
                let v = inline.or_else(|| argv.next()).ok_or("--rotate needs a value (0, 90, 180 or 270)")?;
                rotate = Some(match v.as_str() {
                    "0" => 0,
                    "90" => 90,
                    "180" => 180,
                    "270" => 270,
                    _ => return Err(format!("--rotate must be 0, 90, 180 or 270, not '{v}'")),
                });
            }
            "--cursor" => {
                let v = inline.or_else(|| argv.next()).ok_or("--cursor needs a value (hide or show)")?;
                cursor = Some(match v.as_str() {
                    "hide" => Cursor::Hide,
                    "show" => Cursor::Show,
                    _ => return Err(format!("--cursor must be hide or show, not '{v}'")),
                });
            }
            "--hide-cursor" => {
                no_value(&key, &inline)?;
                cursor = Some(Cursor::Hide);
            }
            "--kiosk" => {
                no_value(&key, &inline)?;
                kiosk = Some(true);
            }
            "--windowed" => {
                no_value(&key, &inline)?;
                kiosk = Some(false);
            }
            "--reset-window" => {
                no_value(&key, &inline)?;
                reset_window = true;
            }
            "--dev-tools" => {
                no_value(&key, &inline)?;
                dev_tools = true;
            }
            other => return Err(format!("unknown argument: {other} (see --help)")),
        }
    }

    // "localhost:5173" -> "http://localhost:5173"
    if !url.contains("://") {
        url = format!("http://{url}");
    }
    let url = Url::parse(&url).map_err(|e| format!("invalid URL '{url}': {e}"))?;
    Ok(Parsed::Run(Args {
        url: with_display_params(url, rotate, cursor),
        kiosk,
        hide_cursor: cursor == Some(Cursor::Hide),
        reset_window,
        dev_tools,
    }))
}

/// Puts `rotate` / `cursor` in the URL's query (which is how the kiosk app is
/// told), replacing any it already had and keeping everything else.
fn with_display_params(mut url: Url, rotate: Option<u16>, cursor: Option<Cursor>) -> Url {
    if rotate.is_none() && cursor.is_none() {
        return url;
    }
    let kept: Vec<(String, String)> = url
        .query_pairs()
        .filter(|(k, _)| !(rotate.is_some() && k == "rotate") && !(cursor.is_some() && k == "cursor"))
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect();
    {
        let mut query = url.query_pairs_mut();
        query.clear().extend_pairs(kept);
        if let Some(r) = rotate {
            query.append_pair("rotate", &r.to_string());
        }
        if let Some(c) = cursor {
            query.append_pair("cursor", c.as_str());
        }
    }
    url
}

fn die(message: &str) -> ! {
    eprintln!("partybox-shell: {message}");
    std::process::exit(2)
}

// --- remembering where the window was --------------------------------------

/// Where the window was, and in which mode, when it was last used. The size and
/// position are the *windowed* ones (kiosk mode overwrites neither), so leaving
/// kiosk mode goes back to where the window was; which monitor kiosk mode covers
/// is the one that position is on. Physical pixels, as the OS reports them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Saved {
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    kiosk: bool,
}

/// x, y, width, height of a monitor, in the same virtual desktop as the windows.
type Rect = (i32, i32, u32, u32);

/// Whether the window's centre is still on a connected monitor (a screen that was
/// unplugged, or a resolution change, can leave the saved spot off every screen).
fn fits_a_monitor(saved: &Saved, monitors: &[Rect]) -> bool {
    if saved.width == 0 || saved.height == 0 {
        return false;
    }
    let cx = saved.x as i64 + saved.width as i64 / 2;
    let cy = saved.y as i64 + saved.height as i64 / 2;
    monitors.iter().any(|&(mx, my, mw, mh)| {
        cx >= mx as i64 && cx < mx as i64 + mw as i64 && cy >= my as i64 && cy < my as i64 + mh as i64
    })
}

/// The mode to start in: what was asked for, else how it was left, else a
/// normal window (kiosk mode only ever on purpose, or on a remembered screen).
fn start_in_kiosk(asked: Option<bool>, restored: Option<&Saved>) -> bool {
    asked.unwrap_or_else(|| restored.is_some_and(|s| s.kiosk))
}

fn load_saved(path: &Path) -> Option<Saved> {
    serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()
}

fn store_saved(path: &Path, saved: &Saved) {
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(json) = serde_json::to_string_pretty(saved) {
        let _ = std::fs::write(path, json);
    }
}

struct Memory {
    path: Option<PathBuf>,
    /// `None` until startup has finished placing the window: the events of that
    /// placement must not be taken for the user moving it.
    state: Mutex<Option<Saved>>,
}

impl Memory {
    fn update(&self, next: Saved) {
        let mut state = self.state.lock().unwrap();
        if state.as_ref() != Some(&next) {
            if let Some(path) = &self.path {
                store_saved(path, &next);
            }
            *state = Some(next);
        }
    }
}

struct Shell {
    url: Url,
    hide_cursor: bool,
    kiosk: AtomicBool,
    memory: Memory,
}

fn geometry(window: &WebviewWindow) -> Option<(i32, i32, u32, u32)> {
    if window.is_minimized().unwrap_or(false) {
        return None; // some systems report a far-away position for minimized windows
    }
    let (pos, size) = (window.outer_position().ok()?, window.inner_size().ok()?);
    (size.width > 0 && size.height > 0).then_some((pos.x, pos.y, size.width, size.height))
}

/// Called whenever the window moves or is resized, and when the mode changes.
fn remember(app: &AppHandle) {
    let (Some(window), Some(shell)) = (app.get_webview_window(WINDOW), app.try_state::<Shell>()) else {
        return;
    };
    let kiosk = shell.kiosk.load(Ordering::SeqCst);
    let Some(previous) = shell.memory.state.lock().unwrap().clone() else {
        return;
    };
    let next = match (kiosk, geometry(&window)) {
        (false, Some((x, y, width, height))) => Saved { x, y, width, height, kiosk },
        _ => Saved { kiosk, ..previous },
    };
    shell.memory.update(next);
}

/// The site the shell was pointed at (the splash page asks for it).
#[tauri::command]
fn target_url(shell: State<'_, Shell>) -> String {
    shell.url.to_string()
}

/// Whether the site's server accepts connections yet.
#[tauri::command]
async fn ping_target(shell: State<'_, Shell>) -> Result<bool, String> {
    let url = shell.url.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let (Some(host), Some(port)) = (url.host_str(), url.port_or_known_default()) else {
            return false;
        };
        (host, port)
            .to_socket_addrs()
            .map(|mut addrs| addrs.any(|a| TcpStream::connect_timeout(&a, Duration::from_secs(2)).is_ok()))
            .unwrap_or(false)
    })
    .await
    .map_err(|e| e.to_string())
}

fn same_origin(a: &Url, b: &Url) -> bool {
    a.scheme() == b.scheme() && a.host() == b.host() && a.port_or_known_default() == b.port_or_known_default()
}

/// The webview may only ever show the local splash page or the configured site
/// (same scheme, host and port): a link on the page must not take a public
/// kiosk somewhere else. `dev_server` is where `cargo tauri dev` serves the
/// splash page from (127.0.0.1:1430) instead of the tauri:// scheme.
fn allowed(target: &Url, dev_server: Option<&Url>, url: &Url) -> bool {
    let is_local_app = url.scheme() == "tauri" || url.host_str() == Some("tauri.localhost") || url.scheme() == "about";
    is_local_app || same_origin(target, url) || dev_server.is_some_and(|d| same_origin(d, url))
}

// macOS: "simple" fullscreen is instant (no new Space, no animation) and hides
// the Dock and the menu bar, which native fullscreen leaves reachable at the
// screen edges. Elsewhere, plain fullscreen already covers everything.
#[cfg(target_os = "macos")]
fn set_fullscreen(window: &WebviewWindow, on: bool) {
    let _ = window.set_simple_fullscreen(on);
}
#[cfg(not(target_os = "macos"))]
fn set_fullscreen(window: &WebviewWindow, on: bool) {
    let _ = window.set_fullscreen(on);
}

fn apply_kiosk(window: &WebviewWindow, on: bool, hide_cursor: bool) {
    if on {
        let _ = window.set_decorations(false);
        // Staying above notifications and panels; not on macOS, where a floating
        // window would also hide the system's camera-permission dialog behind it.
        #[cfg(not(target_os = "macos"))]
        let _ = window.set_always_on_top(true);
        set_fullscreen(window, true);
        let _ = window.hide_menu();
        let _ = window.set_cursor_visible(!hide_cursor);
    } else {
        set_fullscreen(window, false);
        #[cfg(not(target_os = "macos"))]
        let _ = window.set_always_on_top(false);
        let _ = window.set_decorations(true);
        let _ = window.show_menu();
        let _ = window.set_cursor_visible(true);
    }
}

fn toggle_kiosk(app: &AppHandle) {
    let (Some(window), Some(shell)) = (app.get_webview_window(WINDOW), app.try_state::<Shell>()) else {
        return;
    };
    let on = !shell.kiosk.load(Ordering::SeqCst);
    shell.kiosk.store(on, Ordering::SeqCst);
    apply_kiosk(&window, on, shell.hide_cursor);
    remember(app);
    println!("kiosk mode: {}", if on { "on" } else { "off" });
}

// DevTools: the page's right-click menu is disabled (it's a kiosk), so "Inspect
// Element" isn't there; this opens them instead. Cmd+Alt+I is the macOS
// convention, F12 the Linux/Windows one.
#[cfg(target_os = "macos")]
const DEVTOOLS_ACCELERATOR: &str = "Cmd+Alt+I";
#[cfg(not(target_os = "macos"))]
const DEVTOOLS_ACCELERATOR: &str = "F12";

/// Debug builds only: a native menu whose accelerators work while the page has
/// focus (the remote site is given no way to talk to the app), and keep working
/// when the menu bar itself is hidden by kiosk mode. CmdOrCtrl = Cmd on macOS,
/// Ctrl on Linux and Windows.
fn build_dev_menu(app: &AppHandle) -> tauri::Result<()> {
    let toggle = MenuItemBuilder::with_id("toggle-kiosk", "Alternar modo kiosco").accelerator("CmdOrCtrl+Shift+K").build(app)?;
    let reload = MenuItemBuilder::with_id("reload", "Recargar").accelerator("CmdOrCtrl+R").build(app)?;
    let home = MenuItemBuilder::with_id("home", "Volver al inicio").accelerator("CmdOrCtrl+Shift+H").build(app)?;
    let devtools = MenuItemBuilder::with_id("devtools", "Herramientas de desarrollo").accelerator(DEVTOOLS_ACCELERATOR).build(app)?;
    // Our own Quit (not the predefined one) so Ctrl+Q exists on Linux/Windows too.
    let quit = MenuItemBuilder::with_id("quit", "Salir").accelerator("CmdOrCtrl+Q").build(app)?;

    let app_menu = SubmenuBuilder::new(app, "PartyBox Shell").about(None).separator().item(&quit).build()?;
    let kiosk_menu = SubmenuBuilder::new(app, "Kiosco").item(&toggle).item(&reload).item(&home).separator().item(&devtools).build()?;
    let edit_menu = SubmenuBuilder::new(app, "Edición").cut().copy().paste().select_all().build()?;
    app.set_menu(MenuBuilder::new(app).item(&app_menu).item(&kiosk_menu).item(&edit_menu).build()?)?;
    Ok(())
}

fn on_dev_menu(app: &AppHandle, id: &str) {
    match id {
        "toggle-kiosk" => toggle_kiosk(app),
        "quit" => app.exit(0),
        "devtools" => {
            if let Some(w) = app.get_webview_window(WINDOW) {
                w.open_devtools();
            }
        }
        "reload" => {
            if let Some(w) = app.get_webview_window(WINDOW) {
                let _ = w.eval("location.reload()");
            }
        }
        "home" => {
            if let (Some(w), Some(shell)) = (app.get_webview_window(WINDOW), app.try_state::<Shell>()) {
                let target = serde_json::to_string(shell.url.as_str()).unwrap_or_default();
                let _ = w.eval(&format!("location.replace({target})"));
            }
        }
        _ => {}
    }
}

/// Debug builds, `PARTYBOX_SELFTEST=1`: exercises what the shortcut does (same
/// function) and reports the window's state, then quits. Lets kiosk mode be
/// checked without anyone pressing keys on the machine.
#[cfg(debug_assertions)]
fn selftest(app: AppHandle) {
    use std::{thread::sleep, time::Duration};

    fn report(app: &AppHandle, label: &str) {
        let Some(w) = app.get_webview_window(WINDOW) else { return };
        let monitor = w.current_monitor().ok().flatten().map(|m| (m.size().width, m.size().height));
        let outer = w.outer_size().map(|s| (s.width, s.height)).ok();
        println!(
            "selftest [{label}] url={} decorated={:?} outer={:?} monitor={:?} cursor-hidden-flag={} saved={}",
            w.url().map(|u| u.to_string()).unwrap_or_default(),
            w.is_decorated().ok(),
            outer,
            monitor,
            app.try_state::<Shell>().map(|s| s.hide_cursor).unwrap_or(false),
            app.try_state::<Shell>()
                .and_then(|s| s.memory.state.lock().unwrap().clone())
                .map(|s| format!("{}x{}@({},{}) kiosk={}", s.width, s.height, s.x, s.y, s.kiosk))
                .unwrap_or_default(),
        );
    }

    std::thread::spawn(move || {
        sleep(Duration::from_secs(8)); // splash, then the real page
        report(&app, "start");
        // A page trying to send the kiosk elsewhere must go nowhere.
        if let Some(w) = app.get_webview_window(WINDOW) {
            let _ = w.eval("location.href = 'https://example.com/'");
        }
        sleep(Duration::from_secs(3));
        report(&app, "after trying to leave the site");
        toggle_kiosk(&app);
        sleep(Duration::from_secs(2));
        report(&app, "after 1st toggle");
        toggle_kiosk(&app);
        sleep(Duration::from_secs(2));
        report(&app, "after 2nd toggle");
        app.exit(0);
    });
}

fn main() {
    let args = match parse_args(std::env::args().skip(1), std::env::var("PARTYBOX_URL").ok()) {
        Ok(Parsed::Run(args)) => args,
        Ok(Parsed::Help) => {
            println!("{USAGE}");
            return;
        }
        Err(message) => die(&message),
    };

    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![target_url, ping_target])
        .on_menu_event(|app, event| on_dev_menu(app, event.id().as_ref()))
        .on_window_event(|window, event| {
            if matches!(event, WindowEvent::Moved(_) | WindowEvent::Resized(_)) {
                remember(window.app_handle());
            }
        })
        .setup(move |app| {
            let path = app.path().app_config_dir().ok().map(|d| d.join("window.json"));
            app.manage(Shell {
                url: args.url.clone(),
                hide_cursor: args.hide_cursor,
                kiosk: AtomicBool::new(false),
                memory: Memory { path: path.clone(), state: Mutex::new(None) },
            });

            let target = args.url.clone();
            let dev_server = if cfg!(debug_assertions) { app.config().build.dev_url.clone() } else { None };
            // One combined script rather than two `.initialization_script()`
            // calls, to not depend on whether Tauri runs several of those in
            // the order added.
            let init_script = if args.dev_tools {
                format!("{DEV_TOOLS_SCRIPT}\n{KIOSK_SCRIPT}")
            } else {
                KIOSK_SCRIPT.to_string()
            };
            let window = WebviewWindowBuilder::new(app, WINDOW, WebviewUrl::App("index.html".into()))
                .title("PartyBox")
                .inner_size(1200.0, 800.0)
                .initialization_script(&init_script)
                .on_navigation(move |url| {
                    let ok = allowed(&target, dev_server.as_ref(), url);
                    if !ok {
                        eprintln!("blocked navigation to {url}");
                    }
                    ok
                })
                .build()?;

            if cfg!(debug_assertions) {
                build_dev_menu(app.handle())?;
            } else {
                // No menu at all in release builds (macOS would otherwise add a default one).
                app.set_menu(MenuBuilder::new(app).build()?)?;
            }

            // Where it was last time, if that screen is still there.
            if args.reset_window {
                if let Some(path) = &path {
                    let _ = std::fs::remove_file(path);
                }
            }
            let monitors: Vec<Rect> = window
                .available_monitors()
                .unwrap_or_default()
                .iter()
                .map(|m| (m.position().x, m.position().y, m.size().width, m.size().height))
                .collect();
            let remembered = path.as_deref().and_then(load_saved);
            let restored = remembered.filter(|s| fits_a_monitor(s, &monitors));
            if let Some(s) = &restored {
                let _ = window.set_size(PhysicalSize::new(s.width, s.height));
                let _ = window.set_position(PhysicalPosition::new(s.x, s.y));
                println!("window: restored to {}x{} at ({}, {})", s.width, s.height, s.x, s.y);
            } else {
                println!("window: nothing remembered for the connected screens; normal window unless --kiosk");
            }

            let kiosk = start_in_kiosk(args.kiosk, restored.as_ref());
            let shell = app.state::<Shell>();
            shell.kiosk.store(kiosk, Ordering::SeqCst);
            // From here on, moving or resizing the window is remembered.
            let start = restored.or_else(|| {
                geometry(&window).map(|(x, y, width, height)| Saved { x, y, width, height, kiosk })
            });
            if let Some(start) = start {
                shell.memory.update(Saved { kiosk, ..start });
            }
            apply_kiosk(&window, kiosk, args.hide_cursor);
            println!("kiosk mode: {}", if kiosk { "on" } else { "off" });

            #[cfg(debug_assertions)]
            if std::env::var("PARTYBOX_SELFTEST").is_ok() {
                selftest(app.handle().clone());
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running PartyBox Shell");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn u(s: &str) -> Url {
        Url::parse(s).unwrap()
    }

    #[test]
    fn navigation_is_limited_to_the_target_origin_and_the_local_splash() {
        let target = u("http://localhost:5173");
        // the site itself, any path
        assert!(allowed(&target, None, &u("http://localhost:5173/")));
        assert!(allowed(&target, None, &u("http://localhost:5173/anything?x=1#y")));
        // the shell's own splash page (per platform)
        assert!(allowed(&target, None, &u("tauri://localhost/index.html")));
        assert!(allowed(&target, None, &u("http://tauri.localhost/index.html")));
        assert!(allowed(&target, None, &u("about:blank")));
        // anywhere else
        assert!(!allowed(&target, None, &u("https://example.com/")));
        assert!(!allowed(&target, None, &u("http://localhost:5174/")), "another port is another origin");
        assert!(!allowed(&target, None, &u("https://localhost:5173/")), "another scheme is another origin");
        assert!(!allowed(&target, None, &u("http://localhost.evil.com:5173/")));
    }

    #[test]
    fn the_dev_server_origin_is_allowed_only_when_given() {
        let target = u("http://localhost:5173");
        let dev = u("http://127.0.0.1:1430");
        assert!(allowed(&target, Some(&dev), &u("http://127.0.0.1:1430/index.html")));
        assert!(!allowed(&target, None, &u("http://127.0.0.1:1430/index.html")));
        assert!(!allowed(&target, Some(&dev), &u("http://127.0.0.1:9999/")));
    }

    #[test]
    fn default_ports_count_as_the_same_origin() {
        let target = u("https://kiosk.example.com");
        assert!(allowed(&target, None, &u("https://kiosk.example.com:443/x")));
        assert!(!allowed(&target, None, &u("http://kiosk.example.com/x")));
    }

    fn parse(args: &[&str]) -> Result<Args, String> {
        match parse_args(args.iter().map(|s| s.to_string()), None)? {
            Parsed::Run(a) => Ok(a),
            Parsed::Help => Err("help".into()),
        }
    }

    #[test]
    fn options_take_both_forms_and_default_to_the_kiosk_site() {
        let a = parse(&[]).unwrap();
        assert_eq!(a.url.as_str(), "http://localhost:5173/");
        assert_eq!((a.kiosk, a.hide_cursor, a.reset_window, a.dev_tools), (None, false, false, false));
        assert_eq!(parse(&["--url=localhost:8080"]).unwrap().url.as_str(), "http://localhost:8080/");
        assert_eq!(parse(&["--url", "https://k.example.com"]).unwrap().url.as_str(), "https://k.example.com/");
        assert_eq!(parse(&["--kiosk"]).unwrap().kiosk, Some(true));
        assert_eq!(parse(&["--windowed"]).unwrap().kiosk, Some(false));
        assert!(parse(&["--reset-window"]).unwrap().reset_window);
        assert!(parse(&["--dev-tools"]).unwrap().dev_tools);
    }

    #[test]
    fn rotate_and_cursor_are_passed_to_the_site_in_its_url() {
        let a = parse(&["--rotate=90", "--cursor=hide"]).unwrap();
        assert_eq!(a.url.as_str(), "http://localhost:5173/?rotate=90&cursor=hide");
        assert!(a.hide_cursor);
        let b = parse(&["--rotate", "270", "--cursor", "show"]).unwrap();
        assert_eq!(b.url.as_str(), "http://localhost:5173/?rotate=270&cursor=show");
        assert!(!b.hide_cursor);
        assert_eq!(parse(&["--hide-cursor"]).unwrap().url.as_str(), "http://localhost:5173/?cursor=hide");
        assert!(parse(&["--hide-cursor"]).unwrap().hide_cursor);
        // nothing asked, nothing added
        assert_eq!(parse(&["--windowed"]).unwrap().url.as_str(), "http://localhost:5173/");
    }

    #[test]
    fn the_options_replace_what_the_url_already_says_and_keep_the_rest() {
        let a = parse(&["--url=http://h:1/p?a=1&rotate=180&cursor=show#f", "--rotate=90"]).unwrap();
        assert_eq!(a.url.as_str(), "http://h:1/p?a=1&cursor=show&rotate=90#f");
        // the URL's own parameters are left alone when the option isn't given
        let b = parse(&["--url=http://h:1/?rotate=180"]).unwrap();
        assert_eq!(b.url.as_str(), "http://h:1/?rotate=180");
        assert!(!b.hide_cursor);
    }

    #[test]
    fn bad_options_are_refused() {
        for bad in [
            &["--rotate=45"][..],
            &["--rotate"],
            &["--cursor=maybe"],
            &["--windowed=yes"],
            &["--dev-tools=yes"],
            &["--wat"],
            &["kiosk"],
            &["--url=http://"],
        ] {
            assert!(parse(bad).is_err(), "{bad:?} should be refused");
        }
        assert!(matches!(parse_args(["--help".to_string()], None), Ok(Parsed::Help)));
    }

    fn saved(x: i32, y: i32, kiosk: bool) -> Saved {
        Saved { x, y, width: 1200, height: 800, kiosk }
    }

    #[test]
    fn a_remembered_window_counts_only_if_its_screen_is_still_connected() {
        let both = [(0, 0, 1920, 1080), (1920, 0, 1920, 1200)];
        let only_first = [(0, 0, 1920, 1080)];
        assert!(fits_a_monitor(&saved(100, 100, false), &both));
        assert!(fits_a_monitor(&saved(2000, 50, false), &both), "on the second screen");
        assert!(!fits_a_monitor(&saved(2000, 50, false), &only_first), "second screen unplugged");
        assert!(fits_a_monitor(&saved(-300, 100, false), &[(-1920, 0, 1920, 1080), (0, 0, 1920, 1080)]), "screen to the left has negative x");
        assert!(!fits_a_monitor(&saved(9000, 9000, false), &both));
        assert!(!fits_a_monitor(&Saved { width: 0, ..saved(0, 0, false) }, &both));
    }

    #[test]
    fn kiosk_mode_is_never_the_default_without_a_remembered_screen() {
        // nothing remembered: normal window, so it can be moved to the right screen
        assert!(!start_in_kiosk(None, None));
        // remembered: however it was left
        assert!(start_in_kiosk(None, Some(&saved(0, 0, true))));
        assert!(!start_in_kiosk(None, Some(&saved(0, 0, false))));
        // asked for explicitly: wins over what was remembered
        assert!(start_in_kiosk(Some(true), None));
        assert!(!start_in_kiosk(Some(false), Some(&saved(0, 0, true))));
    }

    #[test]
    fn the_remembered_window_survives_a_round_trip_through_the_file() {
        let dir = std::env::temp_dir().join(format!("partybox-shell-test-{}", std::process::id()));
        let path = dir.join("nested").join("window.json");
        assert_eq!(load_saved(&path), None, "no file yet");
        store_saved(&path, &saved(1920, 40, true));
        assert_eq!(load_saved(&path), Some(saved(1920, 40, true)));
        std::fs::write(&path, "not json").unwrap();
        assert_eq!(load_saved(&path), None, "a damaged file is ignored, not fatal");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
