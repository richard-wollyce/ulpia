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
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

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

/// A turning ring, for work with no denominator.
///
/// Generation produces tokens at a knowable rate toward an unknowable total, so a
/// bar there would have to guess, and a bar that guesses lies. A ring that turns
/// says "working" and claims nothing about how much is left. Twelve frames at 80 ms
/// is about one revolution a second, fast enough to read as motion and slow enough
/// not to draw the eye away from what the person is doing.
const SPIN_FRAMES: [&[u8]; 12] = [
    include_bytes!("../icons/spin-00.png"),
    include_bytes!("../icons/spin-01.png"),
    include_bytes!("../icons/spin-02.png"),
    include_bytes!("../icons/spin-03.png"),
    include_bytes!("../icons/spin-04.png"),
    include_bytes!("../icons/spin-05.png"),
    include_bytes!("../icons/spin-06.png"),
    include_bytes!("../icons/spin-07.png"),
    include_bytes!("../icons/spin-08.png"),
    include_bytes!("../icons/spin-09.png"),
    include_bytes!("../icons/spin-10.png"),
    include_bytes!("../icons/spin-11.png"),
];
const SPIN_INTERVAL: Duration = Duration::from_millis(80);

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

/// How many files one drop may carry.
///
/// **Not a context window limit.** Each file is distilled on its own per the
/// ingestion protocol, so they never share a prompt and the window is never the
/// binding constraint.
///
/// The limit exists because ADR-0007 makes every write a proposal a human approves,
/// and a queue of thirty proposals is a queue nobody reads. Someone facing thirty
/// decisions approves all thirty, which is the same as having no gate at all. The
/// bound is the reviewer, not the machine.
const MAX_INGEST_BATCH: usize = 10;

/// What the panel draws, and it lives here rather than in the webview because the
/// panel is dismissed constantly and work outlives it. Closing the panel mid ingest
/// and reopening it has to show where the work got to, which is only possible if the
/// panel is a view over this rather than the owner of it.
#[derive(Clone, Default, Serialize)]
struct Progress {
    /// The named step, rendered as rising text. `None` means nothing is running.
    stage: Option<String>,
    /// Only meaningful for ingestion. `total` of zero means draw no bar, which is
    /// the honest rendering for generation: tokens per second is knowable and the
    /// total is not, and a percentage that guesses is a percentage that lies.
    done: usize,
    total: usize,
    problem: Option<String>,
}

#[derive(Default)]
struct Fleet {
    memory: Mutex<Option<Memory>>,
    root: Mutex<Option<PathBuf>>,
    progress: Mutex<Progress>,
    /// Whether the ring should be turning. Read by the spinner thread every frame.
    spinning: Arc<AtomicBool>,
    /// Whether a spinner thread exists. Separate from `spinning` so a stop and an
    /// immediate restart cannot leave two threads sharing one icon.
    running: Arc<AtomicBool>,
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
    // No index path to pass. Each agent keeps its own at <agent>/.kb/index.db, so
    // there is nothing left to point at the wrong file.
    match Memory::open(&[root.as_path()], false) {
        Ok(memory) => {
            let agents: Vec<String> = memory.agents.iter().map(|a| a.name.clone()).collect();
            let status = Status {
                root: Some(root.display().to_string()),
                agents,
                entries: memory.entry_count(),
                problem: if memory.index_was_rebuilt {
                    Some("Um índice era anterior à correção de privacidade e foi esvaziado. Rode kb index.".into())
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
            agents: memory.agents.iter().map(|a| a.name.clone()).collect(),
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

    stage(&app, &state, "Roteando para o agente…");

    let guard = state.memory.lock().unwrap();
    let memory = match guard.as_ref() {
        Some(m) => m,
        None => {
            failed(&app, &state, "Nenhuma frota aberta.");
            return Err("Nenhuma frota aberta.".into());
        }
    };

    // Before the base is touched. A question about the fleet itself is a lookup, and
    // it stays a lookup: "quem é você?" reduces to the single term `quem` once
    // `normalise` drops the stopwords, and that one term ranked Steve's notes on
    // audience research. Nothing about that was a ranking problem to tune.
    if let Some(a) = memory.identify(&question) {
        done(&app, &state);
        return Ok(vec![Answer {
            agent: memory.fleet_card().name,
            path: "fleet.txt".into(),
            title: String::new(),
            why: "lido da estrutura da frota".into(),
            passages: vec![Passage {
                heading: String::new(),
                text: a.text,
                provenance: Some("fleet.txt, agent.txt, agents/".into()),
            }],
        }]);
    }

    stage(&app, &state, "Lendo a base…");
    let found = memory.retrieve(&question, 5);

    if found.is_empty() {
        // Every exit has to clear the progress. Leaving the tray on a full bar is
        // what made a 283 ms "nothing matched" look like an unbounded wait: the panel
        // explained itself in small text while the tray still showed work running,
        // and the eye goes to the tray. A failure that looks like slowness is worse
        // than a failure that looks like a failure.
        let message = format!("Nada casou com \"{question}\".");
        failed(&app, &state, &message);
        // Said plainly on purpose. A router that always returns something teaches
        // you to trust a guess.
        return Err(message);
    }

    let stale = memory.looks_stale(&found);
    // Nobody agreed means the answer is a guess dressed as a result. Measured: the
    // two questions that routed correctly had both scorers voting; the one that
    // returned marketing psychology for "quem é você?" had one.
    let guessing = memory.no_agreement(&found);
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
        notify(
            &app,
            "Índice desatualizado",
            "Arquivos ranqueados sem passagens. Rode kb index.",
        );
    }

    if guessing {
        publish(
            &app,
            &state,
            Progress {
                stage: None,
                done: 0,
                total: 0,
                problem: Some(
                    "Só um dos dois buscadores encontrou isso, então é palpite e não \
                     resposta. Talvez a base não cubra a pergunta."
                        .into(),
                ),
            },
        );
    } else {
        done(&app, &state);
    }
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
fn accept_files(app: AppHandle, state: State<Fleet>, paths: Vec<String>) -> Result<String, String> {
    if paths.len() > MAX_INGEST_BATCH {
        let message = format!(
            "{} arquivos de uma vez. O limite é {MAX_INGEST_BATCH}, porque cada um vira uma \
             proposta que você aprova, e uma fila longa demais é uma fila que ninguém lê.",
            paths.len()
        );
        failed(&app, &state, &message);
        return Err(message);
    }

    let total = paths.len();
    stage(&app, &state, "Identificando arquivos…");

    // This checks and reports; it does not distil. Ingestion is a model call per file
    // and a proposal per result, and neither exists yet. The bar is over the work
    // actually being done rather than over work being pretended.
    let mut ok = 0usize;
    let mut missing = Vec::new();
    for (i, path) in paths.iter().enumerate() {
        counted(&app, &state, "Identificando arquivos…", i, total);
        match std::fs::metadata(path) {
            Ok(m) if m.is_file() => ok += 1,
            _ => missing.push(
                PathBuf::from(path)
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| path.clone()),
            ),
        }
    }

    done(&app, &state);

    if !missing.is_empty() {
        let message = format!("Não consegui ler: {}", missing.join(", "));
        notify(&app, "Alguns arquivos falharam", &message);
        return Err(message);
    }

    notify(
        &app,
        "Arquivos recebidos",
        &format!("{ok} arquivo{} pronto{} para a inbox.", plural(ok), plural(ok)),
    );
    Ok(format!("{ok} arquivo{} recebido{}", plural(ok), plural(ok)))
}

fn plural(n: usize) -> &'static str {
    if n == 1 { "" } else { "s" }
}

// ---------------------------------------------------------------------------
// Tray feedback
// ---------------------------------------------------------------------------

/// Writes the new state, then emits it with the lock already released.
///
/// Emitting while holding the lock invites a listener that calls back in to wait on
/// a lock its own caller holds, which is a deadlock that only shows up under timing.
fn publish(app: &AppHandle, state: &Fleet, next: Progress) {
    {
        *state.progress.lock().unwrap() = next.clone();
    }

    // The tray is the only progress visible once the panel is dismissed, and leaving
    // it showing work that finished is what made a 283 ms failure look like an
    // endless wait. Three states, and which one applies is decided by whether the
    // work has a denominator rather than by which command is running.
    let counted_work = next.total > 0;
    let uncounted_work = !counted_work && next.stage.is_some();

    // The spinner owns the icon while it runs, so stop it before drawing anything
    // else or the two fight over the tray a frame at a time.
    state.spinning.store(uncounted_work, Ordering::Relaxed);

    if uncounted_work {
        start_spinner(app, state);
    } else if let Some(tray) = app.tray_by_id("fleet") {
        let bytes = if counted_work {
            let frac = (next.done as f64 / next.total as f64).min(1.0);
            PROGRESS_FRAMES[(frac * 10.0).round() as usize]
        } else {
            IDLE_ICON
        };
        if let Ok(icon) = Image::from_bytes(bytes) {
            let _ = tray.set_icon(Some(icon));
        }
    }

    let _ = app.emit("progress", next);
}

/// Turns the ring until the flag drops. Started only when nothing is already
/// turning: two threads on one icon is a stutter, not a faster spin.
fn start_spinner(app: &AppHandle, state: &Fleet) {
    if state.running.swap(true, Ordering::Relaxed) {
        return;
    }

    let app = app.clone();
    let spinning = state.spinning.clone();
    let running = state.running.clone();

    std::thread::spawn(move || {
        let mut frame = 0usize;
        while spinning.load(Ordering::Relaxed) {
            if let Some(tray) = app.tray_by_id("fleet") {
                if let Ok(icon) = Image::from_bytes(SPIN_FRAMES[frame % SPIN_FRAMES.len()]) {
                    let _ = tray.set_icon(Some(icon));
                }
            }
            frame += 1;
            std::thread::sleep(SPIN_INTERVAL);
        }

        // Clear the icon here rather than in `publish`. The thread is what set the
        // last frame, so it is what has to unset it, or a stopped spinner leaves the
        // ring frozen mid turn and the tray still looks busy.
        if let Some(tray) = app.tray_by_id("fleet") {
            if let Ok(icon) = Image::from_bytes(IDLE_ICON) {
                let _ = tray.set_icon(Some(icon));
            }
        }
        running.store(false, Ordering::Relaxed);
    });
}

/// A named step, with no bar. This is the whole feedback for a question: the stages
/// are knowable and the duration is not.
fn stage(app: &AppHandle, state: &Fleet, text: &str) {
    publish(
        app,
        state,
        Progress { stage: Some(text.into()), done: 0, total: 0, problem: None },
    );
}

/// A counted step. Ingestion is the only thing that has a real denominator.
fn counted(app: &AppHandle, state: &Fleet, text: &str, done: usize, total: usize) {
    publish(
        app,
        state,
        Progress { stage: Some(text.into()), done, total, problem: None },
    );
}

fn done(app: &AppHandle, state: &Fleet) {
    publish(app, state, Progress::default());
}

fn failed(app: &AppHandle, state: &Fleet, why: &str) {
    publish(
        app,
        state,
        Progress { stage: None, done: 0, total: 0, problem: Some(why.into()) },
    );
}

/// The panel asks for this when it opens, which is what lets work survive the panel
/// being dismissed. Without it the tray would know and the panel would not.
#[tauri::command]
fn progress_now(state: State<Fleet>) -> Progress {
    state.progress.lock().unwrap().clone()
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
            ask_from_compose,
            progress_now
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
