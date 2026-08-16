// Fleet, the tray shell.
//
// A tray application rather than a window one, deliberately. The thing it has to
// support first is dragging a file onto it, and that means the user is in Explorer
// or a browser when they reach for it. A window that has to be found and focused
// first is a window that loses to just saving the file somewhere.
//
// Two rules follow from that and they are the whole interaction design:
//
//  1. The panel starts hidden and is summoned, never launched.
//  2. **Clicking outside does not close it.** Grabbing a file *is* clicking outside.
//     A panel that closes when it loses focus cannot receive a drop, so the feature
//     and the behaviour are mutually exclusive. Only the tray icon toggles it.
//
// The backend links `kb` as a library rather than speaking MCP to itself, per
// ADR-0009. Retrieval is a function call in this process: no subprocess to spawn,
// monitor, restart or reap, and no serialising to talk to our own code.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::PathBuf;
use std::sync::Mutex;

use kb::memory::Memory;
use serde::Serialize;
use tauri::image::Image;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_autostart::MacosLauncher;
use tauri_plugin_global_shortcut::{Code, Modifiers, Shortcut, ShortcutState};
use tauri_plugin_notification::NotificationExt;

/// One frame per ten percent. A tray icon has no progress API on any platform, so
/// swapping the image is the only mechanism available, not a shortcut.
const PROGRESS_FRAMES: [&[u8]; 11] = [
    include_bytes!("../icons/tray-000.png"),
    include_bytes!("../icons/tray-010.png"),
    include_bytes!("../icons/tray-020.png"),
    include_bytes!("../icons/tray-030.png"),
    include_bytes!("../icons/tray-040.png"),
    include_bytes!("../icons/tray-050.png"),
    include_bytes!("../icons/tray-060.png"),
    include_bytes!("../icons/tray-070.png"),
    include_bytes!("../icons/tray-080.png"),
    include_bytes!("../icons/tray-090.png"),
    include_bytes!("../icons/tray-100.png"),
];
const IDLE_ICON: &[u8] = include_bytes!("../icons/tray.png");

/// The one absolute path in the system, per ADR-0011, and it lives outside the
/// fleet so that moving the fleet is a directory move with nothing to edit inside it.
fn pointer_file(app: &AppHandle) -> Option<PathBuf> {
    app.path().app_config_dir().ok().map(|d| d.join("fleet-root.txt"))
}

fn read_fleet_root(app: &AppHandle) -> Option<PathBuf> {
    let text = std::fs::read_to_string(pointer_file(app)?).ok()?;
    let path = PathBuf::from(text.trim());
    if path.is_dir() { Some(path) } else { None }
}

#[derive(Default)]
struct Fleet {
    memory: Mutex<Option<Memory>>,
    root: Mutex<Option<PathBuf>>,
}

#[derive(Serialize)]
struct Status {
    root: Option<String>,
    agents: Vec<String>,
    entries: usize,
    /// Set when the fleet cannot be opened. Surfaced rather than swallowed: a panel
    /// that silently answers "nothing matched" teaches you the base is empty.
    problem: Option<String>,
}

#[derive(Serialize)]
struct Passage {
    heading: String,
    text: String,
    provenance: Option<String>,
}

#[derive(Serialize)]
struct Answer {
    agent: String,
    path: String,
    title: String,
    why: String,
    passages: Vec<Passage>,
}

// ---------------------------------------------------------------------------
// Opening the fleet
// ---------------------------------------------------------------------------

fn open_fleet(_app: &AppHandle, state: &Fleet, root: PathBuf) -> Status {
    // One index per agent is ADR-0011 and is not built yet, so this opens the
    // shared one for now. Recorded here rather than silently assumed.
    let db = root.join(".kb").join("index.db");

    match Memory::open(&[root.as_path()], false, &db) {
        Ok(memory) => {
            let agents: Vec<String> = memory
                .bases
                .iter()
                .filter_map(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
                .collect();
            let status = Status {
                root: Some(root.display().to_string()),
                agents,
                entries: memory.entry_count(),
                problem: if memory.index_was_rebuilt {
                    Some("The index predated the privacy fix and was emptied. Run kb index.".into())
                } else {
                    None
                },
            };
            *state.memory.lock().unwrap() = Some(memory);
            *state.root.lock().unwrap() = Some(root);
            status
        }
        Err(e) => Status {
            root: Some(root.display().to_string()),
            agents: Vec::new(),
            entries: 0,
            problem: Some(e.to_string()),
        },
    }
}

#[tauri::command]
fn status(app: AppHandle, state: State<Fleet>) -> Status {
    if let Some(memory) = state.memory.lock().unwrap().as_ref() {
        return Status {
            root: state.root.lock().unwrap().as_ref().map(|p| p.display().to_string()),
            agents: memory
                .bases
                .iter()
                .filter_map(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
                .collect(),
            entries: memory.entry_count(),
            problem: None,
        };
    }

    match read_fleet_root(&app) {
        Some(root) => open_fleet(&app, &state, root),
        None => Status {
            root: None,
            agents: Vec::new(),
            entries: 0,
            problem: Some("No fleet chosen yet.".into()),
        },
    }
}

/// Points at a fleet root and remembers it. Asking is deliberate: a tool that
/// invents a location when it cannot find the old one is how a user ends up with
/// two fleets and notices months later.
#[tauri::command]
fn set_fleet_root(app: AppHandle, state: State<Fleet>, path: String) -> Status {
    let root = PathBuf::from(path);
    if let Some(file) = pointer_file(&app) {
        if let Some(parent) = file.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(file, root.display().to_string());
    }
    open_fleet(&app, &state, root)
}

// ---------------------------------------------------------------------------
// Asking
// ---------------------------------------------------------------------------

#[tauri::command]
fn ask(app: AppHandle, state: State<Fleet>, question: String) -> Result<Vec<Answer>, String> {
    let question = question.trim().to_string();
    if question.is_empty() {
        return Err("Type a question first.".into());
    }

    progress(&app, 20);

    let guard = state.memory.lock().unwrap();
    let memory = guard.as_ref().ok_or("No fleet is open.")?;

    progress(&app, 60);
    let found = memory.retrieve(&question, 5);
    progress(&app, 100);

    if found.is_empty() {
        // The icon has to come back to idle here too. Leaving it at a full progress
        // bar is what made a 283 ms "nothing matched" look like an unbounded wait:
        // the panel said so in small text while the tray still showed work in
        // progress, and the tray is what the eye goes to. A failure that looks like
        // slowness is worse than a failure that looks like a failure.
        idle(&app);
        // Said plainly on purpose. A router that always returns something teaches
        // you to trust a guess.
        return Err(format!("Nada casou com \"{question}\"."));
    }

    let stale = memory.looks_stale(&found);
    let answers = found
        .into_iter()
        .map(|f| Answer {
            agent: f.base,
            path: f.path,
            title: f.title,
            why: f.why.join(" + "),
            passages: f
                .passages
                .into_iter()
                .map(|p| Passage {
                    heading: p.heading_path,
                    text: p.text,
                    provenance: p.provenance,
                })
                .collect(),
        })
        .collect();

    if stale {
        notify(&app, "Index looks stale", "Files ranked but no passages. Run kb index.");
    }

    idle(&app);
    Ok(answers)
}

#[tauri::command]
fn open_compose(app: AppHandle) {
    if let Some(window) = app.get_webview_window("compose") {
        let _ = window.show();
        let _ = window.set_focus();
    }
}

/// The compose window hands its text to the panel rather than rendering results
/// itself. One place draws answers, so the two windows cannot drift apart.
#[tauri::command]
fn ask_from_compose(app: AppHandle, question: String) {
    if let Some(panel) = app.get_webview_window("panel") {
        let _ = panel.show();
        let _ = panel.set_focus();
    }
    let _ = app.emit("answer", question);
}

/// Files dropped on the panel. They are reported, not ingested: distillation is a
/// judgement call and ADR-0007 keeps the write side a proposal.
#[tauri::command]
fn accept_files(app: AppHandle, paths: Vec<String>) -> String {
    let n = paths.len();
    notify(
        &app,
        "Files received",
        &format!("{n} file{} queued for the inbox.", if n == 1 { "" } else { "s" }),
    );
    format!("{n} received")
}

// ---------------------------------------------------------------------------
// Tray feedback
// ---------------------------------------------------------------------------

fn progress(app: &AppHandle, percent: u8) {
    let frame = (percent.min(100) / 10) as usize;
    if let Some(tray) = app.tray_by_id("fleet") {
        if let Ok(icon) = Image::from_bytes(PROGRESS_FRAMES[frame]) {
            let _ = tray.set_icon(Some(icon));
        }
    }
    let _ = app.emit("progress", percent);
}

fn idle(app: &AppHandle) {
    if let Some(tray) = app.tray_by_id("fleet") {
        if let Ok(icon) = Image::from_bytes(IDLE_ICON) {
            let _ = tray.set_icon(Some(icon));
        }
    }
}

fn notify(app: &AppHandle, title: &str, body: &str) {
    let _ = app.notification().builder().title(title).body(body).show();
}

// ---------------------------------------------------------------------------
// Windows
// ---------------------------------------------------------------------------

fn toggle(app: &AppHandle, label: &str, near_tray: Option<(f64, f64)>) {
    let Some(window) = app.get_webview_window(label) else { return };

    if window.is_visible().unwrap_or(false) {
        let _ = window.hide();
        return;
    }

    if let Some((x, y)) = near_tray {
        if let Ok(size) = window.outer_size() {
            // Sit above the tray icon, which is where the taskbar is on Windows by
            // default. Clamped so the panel cannot open with half of it off screen.
            let mut px = x - (size.width as f64) / 2.0;
            let mut py = y - (size.height as f64) - 12.0;
            if let Ok(Some(monitor)) = window.current_monitor() {
                let area = monitor.size();
                px = px.max(8.0).min(area.width as f64 - size.width as f64 - 8.0);
                py = py.max(8.0);
            }
            let _ = window.set_position(tauri::PhysicalPosition::new(px, py));
        }
    }

    let _ = window.show();
    let _ = window.set_focus();
}

fn main() {
    // Ctrl+Shift+Space summons the compose window from anywhere, which is the point
    // of a tray app: the thought arrives while you are in another program, and a
    // shortcut that only works when the panel already has focus is no shortcut.
    //
    // Space rather than a letter because letter combinations collide with editors
    // and browsers far more often, and a global shortcut that steals a key from
    // something the user already relies on is a bug they will blame on us.
    let summon = Shortcut::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::Space);
    let for_handler = summon;

    let shortcuts = tauri_plugin_global_shortcut::Builder::new()
        .with_handler(move |app, pressed, event| {
            // Fires on both press and release. Acting on both toggles twice and the
            // window appears to do nothing at all.
            if pressed == &for_handler && event.state() == ShortcutState::Pressed {
                toggle(app, "compose", None);
            }
        })
        .build();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(shortcuts)
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            None,
        ))
        .manage(Fleet::default())
        .invoke_handler(tauri::generate_handler![
            status,
            set_fleet_root,
            ask,
            accept_files,
            open_compose,
            ask_from_compose
        ])
        .setup(move |app| {
            let handle = app.handle().clone();

            let open = MenuItem::with_id(app, "open", "Open panel", true, None::<&str>)?;
            let compose = MenuItem::with_id(app, "compose", "Write a question", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(
                app,
                &[&open, &compose, &PredefinedMenuItem::separator(app)?, &quit],
            )?;

            TrayIconBuilder::with_id("fleet")
                .icon(Image::from_bytes(IDLE_ICON)?)
                .icon_as_template(true)
                .tooltip("Fleet")
                .menu(&menu)
                // False, or the left click is swallowed by the menu and the panel
                // can never be summoned with one click.
                .show_menu_on_left_click(false)
                .on_menu_event(move |app, event| match event.id.as_ref() {
                    "open" => toggle(app, "panel", None),
                    "compose" => toggle(app, "compose", None),
                    "quit" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(move |tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        rect,
                        ..
                    } = event
                    {
                        // Position and Size are physical-or-logical enums rather
                        // than plain structs, and mixing the two units is how a
                        // panel lands off screen on a scaled display.
                        let anchor = match (rect.position, rect.size) {
                            (tauri::Position::Physical(p), tauri::Size::Physical(s)) => {
                                Some((p.x as f64 + s.width as f64 / 2.0, p.y as f64))
                            }
                            (tauri::Position::Logical(p), tauri::Size::Logical(s)) => {
                                Some((p.x + s.width / 2.0, p.y))
                            }
                            _ => None,
                        };
                        toggle(tray.app_handle(), "panel", anchor);
                    }
                })
                .build(app)?;

            // Registered after the plugin is initialised, and the result is reported
            // rather than swallowed: a shortcut another program already owns fails
            // here, and failing in silence means the user presses it forever.
            use tauri_plugin_global_shortcut::GlobalShortcutExt;
            if let Err(e) = handle.global_shortcut().register(summon) {
                eprintln!("fleet: could not register Ctrl+Shift+Space: {e}");
            }

            // Opening at startup means the first question does not pay for discovery
            // and the panel can say what it is serving before being asked.
            if let Some(root) = read_fleet_root(&handle) {
                let state = handle.state::<Fleet>();
                open_fleet(&handle, &state, root);
            }

            Ok(())
        })
        .on_window_event(|window, event| {
            // The close button hides rather than exits. A tray app that quits when a
            // panel is dismissed is a tray app the user has to relaunch constantly.
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .run(tauri::generate_context!())
        .expect("fleet tray failed to start");
}
