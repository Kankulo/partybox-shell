// PartyBox Shell: a window that is nothing but a webview pointed at one URL, and
// that can take over the whole screen ("kiosk mode") with no bars of any kind.
//
//   partybox-shell [--url <url>] [--windowed] [--hide-cursor]
//
//   --url <url>      the site to show (or env PARTYBOX_URL). Default http://localhost:5173
//   --windowed       start as a normal window instead of in kiosk mode
//   --hide-cursor    hide the mouse pointer in kiosk mode (touch screens)
//
// In debug builds there's a native menu with a shortcut to enter/leave kiosk
// mode (Cmd/Ctrl+Shift+K), reload and quit, so it can be worked on comfortably.
// Release builds have no menu and no way out of kiosk mode: that's the point.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::net::{TcpStream, ToSocketAddrs};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use tauri::menu::{MenuBuilder, MenuItemBuilder, SubmenuBuilder};
use tauri::{AppHandle, Manager, State, Url, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

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

struct Shell {
    url: Url,
    hide_cursor: bool,
    kiosk: AtomicBool,
}

struct Args {
    url: Url,
    windowed: bool,
    hide_cursor: bool,
}

fn parse_args() -> Args {
    let mut url = std::env::var("PARTYBOX_URL").unwrap_or_else(|_| DEFAULT_URL.into());
    let mut windowed = false;
    let mut hide_cursor = false;

    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--url" => url = args.next().unwrap_or_else(|| die("--url needs a value")),
            "--windowed" => windowed = true,
            "--hide-cursor" => hide_cursor = true,
            other => die(&format!("unknown argument: {other}")),
        }
    }

    // "localhost:5173" -> "http://localhost:5173"
    if !url.contains("://") {
        url = format!("http://{url}");
    }
    let url = Url::parse(&url).unwrap_or_else(|e| die(&format!("invalid URL '{url}': {e}")));
    Args { url, windowed, hide_cursor }
}

fn die(message: &str) -> ! {
    eprintln!("partybox-shell: {message}");
    std::process::exit(2)
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

/// The webview may only ever show the local splash page or the configured site
/// (same scheme, host and port): a link on the page must not take a public
/// kiosk somewhere else.
fn allowed(target: &Url, url: &Url) -> bool {
    let is_local_app = url.scheme() == "tauri" || url.host_str() == Some("tauri.localhost") || url.scheme() == "about";
    let same_origin = url.scheme() == target.scheme() && url.host() == target.host() && url.port_or_known_default() == target.port_or_known_default();
    is_local_app || same_origin
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
    println!("kiosk mode: {}", if on { "on" } else { "off" });
}

/// Debug builds only: a native menu whose accelerators work while the page has
/// focus (the remote site is given no way to talk to the app), and keep working
/// when the menu bar itself is hidden by kiosk mode.
fn build_dev_menu(app: &AppHandle) -> tauri::Result<()> {
    let toggle = MenuItemBuilder::with_id("toggle-kiosk", "Alternar modo kiosco").accelerator("CmdOrCtrl+Shift+K").build(app)?;
    let reload = MenuItemBuilder::with_id("reload", "Recargar").accelerator("CmdOrCtrl+R").build(app)?;
    let home = MenuItemBuilder::with_id("home", "Volver al inicio").accelerator("CmdOrCtrl+Shift+H").build(app)?;

    let app_menu = SubmenuBuilder::new(app, "PartyBox Shell").about(None).separator().quit().build()?;
    let kiosk_menu = SubmenuBuilder::new(app, "Kiosco").item(&toggle).item(&reload).item(&home).build()?;
    let edit_menu = SubmenuBuilder::new(app, "Edición").cut().copy().paste().select_all().build()?;
    app.set_menu(MenuBuilder::new(app).item(&app_menu).item(&kiosk_menu).item(&edit_menu).build()?)?;
    Ok(())
}

fn on_dev_menu(app: &AppHandle, id: &str) {
    match id {
        "toggle-kiosk" => toggle_kiosk(app),
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
            "selftest [{label}] url={} decorated={:?} outer={:?} monitor={:?} cursor-hidden-flag={}",
            w.url().map(|u| u.to_string()).unwrap_or_default(),
            w.is_decorated().ok(),
            outer,
            monitor,
            app.try_state::<Shell>().map(|s| s.hide_cursor).unwrap_or(false),
        );
    }

    std::thread::spawn(move || {
        sleep(Duration::from_secs(8)); // splash, then the real page
        report(&app, "start");
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
    let args = parse_args();
    let kiosk_at_start = !args.windowed;

    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![target_url, ping_target])
        .on_menu_event(|app, event| on_dev_menu(app, event.id().as_ref()))
        .setup(move |app| {
            app.manage(Shell { url: args.url.clone(), hide_cursor: args.hide_cursor, kiosk: AtomicBool::new(kiosk_at_start) });

            let target = args.url.clone();
            let window = WebviewWindowBuilder::new(app, WINDOW, WebviewUrl::App("index.html".into()))
                .title("PartyBox")
                .inner_size(1200.0, 800.0)
                .initialization_script(KIOSK_SCRIPT)
                .on_navigation(move |url| {
                    let ok = allowed(&target, url);
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

            apply_kiosk(&window, kiosk_at_start, args.hide_cursor);

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
        assert!(allowed(&target, &u("http://localhost:5173/")));
        assert!(allowed(&target, &u("http://localhost:5173/anything?x=1#y")));
        // the shell's own splash page (per platform)
        assert!(allowed(&target, &u("tauri://localhost/index.html")));
        assert!(allowed(&target, &u("http://tauri.localhost/index.html")));
        assert!(allowed(&target, &u("about:blank")));
        // anywhere else
        assert!(!allowed(&target, &u("https://example.com/")));
        assert!(!allowed(&target, &u("http://localhost:5174/")), "another port is another origin");
        assert!(!allowed(&target, &u("https://localhost:5173/")), "another scheme is another origin");
        assert!(!allowed(&target, &u("http://localhost.evil.com:5173/")));
    }

    #[test]
    fn default_ports_count_as_the_same_origin() {
        let target = u("https://kiosk.example.com");
        assert!(allowed(&target, &u("https://kiosk.example.com:443/x")));
        assert!(!allowed(&target, &u("http://kiosk.example.com/x")));
    }
}
