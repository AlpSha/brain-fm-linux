use std::sync::mpsc::{self, Sender};
use std::sync::Mutex;

use souvlaki::{MediaControlEvent, MediaControls, MediaMetadata, MediaPlayback, PlatformConfig};
use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    webview::NewWindowResponse,
    AppHandle, Manager, State, WebviewUrl, WebviewWindowBuilder, WindowEvent,
};

const BRAIN_FM_URL: &str = "https://my.brain.fm";
const UA: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36 BrainFmDesktop";

/// Snapshot of the player pushed from the webview to the MPRIS thread.
struct NowPlaying {
    playing: bool,
    title: String,
    artist: String,
    album: String,
    art: String,
}

/// Channel the JS bridge writes to (via the `update_now_playing` command).
type MprisTx = Mutex<Option<Sender<NowPlaying>>>;

// ---- helpers --------------------------------------------------------------

fn eval_control(app: &AppHandle, action: &str) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.eval(format!("window.__brainfm && window.__brainfm('{action}')"));
    }
}

fn show_window(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

fn toggle_window(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        if w.is_visible().unwrap_or(false) && !w.is_minimized().unwrap_or(false) {
            let _ = w.hide();
        } else {
            show_window(app);
        }
    }
}

// ---- command --------------------------------------------------------------

#[tauri::command]
#[allow(clippy::too_many_arguments)]
fn update_now_playing(
    tx: State<'_, MprisTx>,
    playing: bool,
    title: String,
    artist: String,
    album: String,
    art_url: String,
) {
    if let Ok(guard) = tx.lock() {
        if let Some(sender) = guard.as_ref() {
            let _ = sender.send(NowPlaying {
                playing,
                title,
                artist,
                album,
                art: art_url,
            });
        }
    }
}

// ---- MPRIS ----------------------------------------------------------------

/// Spawns the MPRIS service. Returns a Sender the command handler can push
/// state to. If MPRIS can't start (e.g. no D-Bus) the app keeps working
/// without media-key support.
fn start_mpris(app: AppHandle) -> Option<Sender<NowPlaying>> {
    let config = PlatformConfig {
        dbus_name: "brainfm",
        display_name: "Brain.fm",
        hwnd: None,
    };

    let mut controls = match MediaControls::new(config) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("[brain-fm] MPRIS unavailable: {e:?}");
            return None;
        }
    };

    let app_evt = app.clone();
    if let Err(e) = controls.attach(move |event: MediaControlEvent| match event {
        MediaControlEvent::Play => eval_control(&app_evt, "play"),
        MediaControlEvent::Pause => eval_control(&app_evt, "pause"),
        MediaControlEvent::Toggle => eval_control(&app_evt, "toggle"),
        MediaControlEvent::Next => eval_control(&app_evt, "next"),
        MediaControlEvent::Previous => eval_control(&app_evt, "prev"),
        MediaControlEvent::Stop => eval_control(&app_evt, "stop"),
        MediaControlEvent::Raise => show_window(&app_evt),
        MediaControlEvent::Quit => app_evt.exit(0),
        _ => {}
    }) {
        eprintln!("[brain-fm] MPRIS attach failed: {e:?}");
        return None;
    }

    let _ = controls.set_playback(MediaPlayback::Paused { progress: None });

    let (tx, rx) = mpsc::channel::<NowPlaying>();

    // Owner thread keeps `controls` alive and applies updates.
    std::thread::spawn(move || {
        let mut controls = controls;
        for np in rx {
            let _ = controls.set_metadata(MediaMetadata {
                title: Some(&np.title),
                artist: Some(&np.artist),
                album: if np.album.is_empty() {
                    None
                } else {
                    Some(&np.album)
                },
                cover_url: if np.art.is_empty() {
                    None
                } else {
                    Some(&np.art)
                },
                duration: None,
            });
            let _ = controls.set_playback(if np.playing {
                MediaPlayback::Playing { progress: None }
            } else {
                MediaPlayback::Paused { progress: None }
            });
        }
    });

    Some(tx)
}

// ---- tray -----------------------------------------------------------------

fn build_tray(app: &AppHandle) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, "show", "Show / Hide Brain.fm", true, None::<&str>)?;
    let playpause = MenuItem::with_id(app, "playpause", "Play / Pause", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(
        app,
        &[
            &show,
            &playpause,
            &PredefinedMenuItem::separator(app)?,
            &quit,
        ],
    )?;

    let mut builder = TrayIconBuilder::with_id("main")
        .tooltip("Brain.fm")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => toggle_window(app),
            "playpause" => eval_control(app, "toggle"),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                toggle_window(tray.app_handle());
            }
        });

    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }

    let tray = builder.build(app)?;
    app.manage(tray);
    Ok(())
}

// ---- entry ----------------------------------------------------------------

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage::<MprisTx>(Mutex::new(None))
        .invoke_handler(tauri::generate_handler![update_now_playing])
        .setup(|app| {
            let handle = app.handle().clone();

            let win = WebviewWindowBuilder::new(
                app,
                "main",
                WebviewUrl::External(BRAIN_FM_URL.parse().unwrap()),
            )
            .title("Brain.fm")
            .inner_size(1200.0, 820.0)
            .min_inner_size(420.0, 560.0)
            .center()
            .user_agent(UA)
            // Social logins (Sign in with Apple / Google / Facebook) open their
            // auth flow in a `window.open()` popup. Without this handler Tauri
            // denies the popup and the login silently fails. Allowing it makes
            // wry open the popup as a *related* WebKitGTK view that shares the
            // cookie/session store, so the login propagates back to the app.
            .on_new_window(|url, _features| {
                eprintln!("[brain-fm] popup requested: {url}");
                NewWindowResponse::Allow
            })
            .devtools(true)
            .initialization_script(include_str!("control.js"))
            .build()?;

            // Close button hides to tray instead of quitting.
            let hide_handle = handle.clone();
            win.on_window_event(move |event| {
                if let WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    if let Some(w) = hide_handle.get_webview_window("main") {
                        let _ = w.hide();
                    }
                }
            });

            build_tray(&handle)?;

            if let Some(tx) = start_mpris(handle.clone()) {
                let state: State<'_, MprisTx> = app.state();
                *state.lock().unwrap() = Some(tx);
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running brain-fm");
}
