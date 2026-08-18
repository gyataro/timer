//! Native timer backend for programmable Timer DSL programs.
//!
//! The frontend parses YAML into a validated program definition. This backend
//! validates the definition again, stores it in the application data directory,
//! and evaluates repeated blocks lazily while keeping the timer deadline
//! authoritative when the window is hidden or suspended.

use std::{
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Condvar, Mutex},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    Emitter, Manager, State,
};

const MAX_SOURCE_BYTES: usize = 1024 * 1024;
const MAX_PROGRAM_ENTRIES: usize = 10_000;
const FLUENT_COLORS: &[&str] = &[
    "darkRed",
    "burgundy",
    "cranberry",
    "red",
    "darkOrange",
    "bronze",
    "pumpkin",
    "orange",
    "peach",
    "marigold",
    "yellow",
    "gold",
    "brass",
    "brown",
    "darkBrown",
    "lime",
    "forest",
    "seafoam",
    "lightGreen",
    "green",
    "darkGreen",
    "lightTeal",
    "teal",
    "darkTeal",
    "cyan",
    "steel",
    "lightBlue",
    "blue",
    "royalBlue",
    "darkBlue",
    "cornflower",
    "navy",
    "lavender",
    "purple",
    "darkPurple",
    "orchid",
    "grape",
    "berry",
    "lilac",
    "pink",
    "hotPink",
    "magenta",
    "plum",
    "beige",
    "mink",
    "silver",
    "platinum",
    "anchor",
    "charcoal",
];

#[derive(Clone, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct Activity {
    title: String,
    duration: u64,
    color: String,
}

#[derive(Clone, serde::Deserialize, serde::Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
enum ProgramEntry {
    Activity {
        title: String,
        duration: u64,
        color: String,
    },
    Repeat {
        count: Option<u64>,
        activities: Vec<Activity>,
    },
}

#[derive(Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProgramDefinition {
    name: String,
    entries: Vec<ProgramEntry>,
}

#[derive(Clone, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct StoredProgram {
    id: String,
    name: String,
    source: String,
    entries: Vec<ProgramEntry>,
}

#[derive(serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct StoredLibrary {
    selected_id: String,
    programs: Vec<StoredProgram>,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ProgramSummary {
    id: String,
    name: String,
    selected: bool,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ProgramLibrary {
    selected_id: String,
    programs: Vec<ProgramSummary>,
}

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct TimerUpdate {
    program_id: String,
    program_name: String,
    activity_name: String,
    color: String,
    remaining: u64,
    paused: bool,
    running: bool,
    completed: bool,
}

#[derive(Clone, Copy, Default)]
struct Cursor {
    entry: usize,
    repetition: u64,
    activity: usize,
}

#[derive(Clone)]
struct Timer {
    cursor: Cursor,
    deadline: Instant,
    paused_for: Duration,
    paused: bool,
    running: bool,
    completed: bool,
    revision: u64,
}

struct AppState {
    programs: Vec<StoredProgram>,
    selected_id: String,
    storage_path: Option<PathBuf>,
    timer: Timer,
}

struct Shared(Arc<(Mutex<AppState>, Condvar)>);

fn default_program() -> StoredProgram {
    StoredProgram {
        id: "default-20-20-20".into(),
        name: "20-20-20".into(),
        source: concat!(
            "20-20-20:\n",
            "  - forever:\n",
            "      - 20m Work: blue\n",
            "      - 20s Break: green\n",
        )
        .into(),
        entries: vec![ProgramEntry::Repeat {
            count: None,
            activities: vec![
                Activity {
                    title: "Work".into(),
                    duration: 20 * 60,
                    color: "blue".into(),
                },
                Activity {
                    title: "Break".into(),
                    duration: 20,
                    color: "green".into(),
                },
            ],
        }],
    }
}

fn initial_state() -> AppState {
    let program = default_program();
    let duration = Duration::from_secs(20 * 60);
    AppState {
        selected_id: program.id.clone(),
        programs: vec![program],
        storage_path: None,
        timer: Timer {
            cursor: Cursor::default(),
            deadline: Instant::now() + duration,
            paused_for: duration,
            paused: false,
            running: false,
            completed: false,
            revision: 0,
        },
    }
}

fn as_activity(entry: &ProgramEntry) -> Option<Activity> {
    match entry {
        ProgramEntry::Activity {
            title,
            duration,
            color,
        } => Some(Activity {
            title: title.clone(),
            duration: *duration,
            color: color.clone(),
        }),
        ProgramEntry::Repeat { .. } => None,
    }
}

fn current_activity<'a>(program: &'a StoredProgram, cursor: &Cursor) -> Option<&'a Activity> {
    match program.entries.get(cursor.entry)? {
        ProgramEntry::Activity { .. } => None,
        ProgramEntry::Repeat { activities, .. } => activities.get(cursor.activity),
    }
}

fn current_activity_owned(program: &StoredProgram, cursor: &Cursor) -> Option<Activity> {
    let entry = program.entries.get(cursor.entry)?;
    as_activity(entry).or_else(|| current_activity(program, cursor).cloned())
}

fn advance_cursor(program: &StoredProgram, cursor: &mut Cursor) -> Option<Activity> {
    match program.entries.get(cursor.entry)? {
        ProgramEntry::Activity { .. } => {
            cursor.entry += 1;
            cursor.repetition = 0;
            cursor.activity = 0;
        }
        ProgramEntry::Repeat { count, activities } => {
            if cursor.activity + 1 < activities.len() {
                cursor.activity += 1;
            } else {
                cursor.activity = 0;
                match count {
                    None => cursor.repetition = cursor.repetition.wrapping_add(1),
                    Some(total) if cursor.repetition + 1 < *total => cursor.repetition += 1,
                    Some(_) => {
                        cursor.entry += 1;
                        cursor.repetition = 0;
                    }
                }
            }
        }
    }
    current_activity_owned(program, cursor)
}

fn selected_program(state: &AppState) -> &StoredProgram {
    state
        .programs
        .iter()
        .find(|program| program.id == state.selected_id)
        .unwrap_or(&state.programs[0])
}

fn reset_timer(state: &mut AppState) {
    let activity = current_activity_owned(selected_program(state), &Cursor::default())
        .expect("validated program has a first activity");
    let duration = Duration::from_secs(activity.duration);
    let revision = state.timer.revision.wrapping_add(1);
    state.timer = Timer {
        cursor: Cursor::default(),
        deadline: Instant::now() + duration,
        paused_for: duration,
        paused: false,
        running: false,
        completed: false,
        revision,
    };
}

fn seconds_ceil(duration: Duration) -> u64 {
    duration
        .as_secs()
        .saturating_add(u64::from(duration.subsec_nanos() > 0))
}

fn snapshot(state: &AppState) -> TimerUpdate {
    let program = selected_program(state);
    let activity = current_activity_owned(program, &state.timer.cursor).unwrap_or(Activity {
        title: "Complete".into(),
        duration: 0,
        color: "blue".into(),
    });
    let remaining = if state.timer.completed {
        0
    } else if state.timer.paused {
        seconds_ceil(state.timer.paused_for)
    } else if state.timer.running {
        seconds_ceil(
            state
                .timer
                .deadline
                .saturating_duration_since(Instant::now()),
        )
    } else {
        activity.duration
    };
    TimerUpdate {
        program_id: program.id.clone(),
        program_name: program.name.clone(),
        activity_name: activity.title,
        color: activity.color,
        remaining,
        paused: state.timer.paused,
        running: state.timer.running,
        completed: state.timer.completed,
    }
}

fn library_snapshot(state: &AppState) -> ProgramLibrary {
    ProgramLibrary {
        selected_id: state.selected_id.clone(),
        programs: state
            .programs
            .iter()
            .map(|program| ProgramSummary {
                id: program.id.clone(),
                name: program.name.clone(),
                selected: program.id == state.selected_id,
            })
            .collect(),
    }
}

fn validate_activity(activity: &Activity) -> Result<(), String> {
    if activity.title.trim().is_empty() {
        return Err("An activity title cannot be empty.".into());
    }
    if activity.duration == 0 {
        return Err(format!(
            "Activity \"{}\" has a zero duration.",
            activity.title
        ));
    }
    if !FLUENT_COLORS.contains(&activity.color.as_str()) {
        return Err(format!(
            "Activity \"{}\" uses unsupported Fluent color \"{}\".",
            activity.title, activity.color
        ));
    }
    Ok(())
}

fn validate_program(program: &StoredProgram) -> Result<(), String> {
    if program.name.trim().is_empty() {
        return Err("The program name cannot be empty.".into());
    }
    if program.source.len() > MAX_SOURCE_BYTES {
        return Err("Timer programs must be no larger than 1 MiB.".into());
    }
    if program.entries.is_empty() {
        return Err("A program must contain at least one entry.".into());
    }
    if program.entries.len() > MAX_PROGRAM_ENTRIES {
        return Err("The program contains too many entries.".into());
    }

    for (index, entry) in program.entries.iter().enumerate() {
        match entry {
            ProgramEntry::Activity {
                title,
                duration,
                color,
            } => validate_activity(&Activity {
                title: title.clone(),
                duration: *duration,
                color: color.clone(),
            })?,
            ProgramEntry::Repeat { count, activities } => {
                if count == &Some(0) {
                    return Err("A repetition count must be at least 1x.".into());
                }
                if activities.is_empty() {
                    return Err("A repeated block must contain an activity.".into());
                }
                if count.is_none() && index + 1 != program.entries.len() {
                    return Err("The forever block must be the final program entry.".into());
                }
                for activity in activities {
                    validate_activity(activity)?;
                }
            }
        }
    }
    Ok(())
}

fn persist(state: &AppState) -> Result<(), String> {
    let Some(path) = &state.storage_path else {
        return Ok(());
    };
    let library = StoredLibrary {
        selected_id: state.selected_id.clone(),
        programs: state.programs.clone(),
    };
    let json = serde_json::to_vec_pretty(&library).map_err(|error| error.to_string())?;
    fs::write(path, json).map_err(|error| format!("Could not save programs: {error}"))
}

fn load_library(path: &Path) -> Option<StoredLibrary> {
    let bytes = fs::read(path).ok()?;
    let mut library: StoredLibrary = serde_json::from_slice(&bytes).ok()?;
    let mut ids = Vec::<String>::new();
    let mut names = Vec::<String>::new();
    library.programs.retain(|program| {
        let valid = validate_program(program).is_ok()
            && !ids.contains(&program.id)
            && !names
                .iter()
                .any(|name| name.eq_ignore_ascii_case(&program.name));
        if valid {
            ids.push(program.id.clone());
            names.push(program.name.clone());
        }
        valid
    });
    if library.programs.is_empty() {
        return None;
    }
    if !library
        .programs
        .iter()
        .any(|program| program.id == library.selected_id)
    {
        library.selected_id = library.programs[0].id.clone();
    }
    Some(library)
}

fn emit_update(app: &tauri::AppHandle, state: &AppState) {
    let _ = app.emit("timer-update", snapshot(state));
}

#[tauri::command]
fn timer_action(action: &str, shared: State<'_, Shared>, app: tauri::AppHandle) -> TimerUpdate {
    let (lock, wake) = &*shared.0;
    let mut state = lock.lock().unwrap();
    let mut changed = false;

    match action {
        "play" => {
            if state.timer.completed {
                reset_timer(&mut state);
            }
            let activity = current_activity_owned(selected_program(&state), &state.timer.cursor)
                .expect("validated cursor points to an activity");
            let duration = Duration::from_secs(activity.duration);
            state.timer.running = true;
            state.timer.paused = false;
            state.timer.paused_for = duration;
            state.timer.deadline = Instant::now() + duration;
            changed = true;
        }
        "pause" if state.timer.running && !state.timer.paused => {
            state.timer.paused_for = state
                .timer
                .deadline
                .saturating_duration_since(Instant::now());
            state.timer.paused = true;
            changed = true;
        }
        "resume" if state.timer.paused => {
            state.timer.deadline = Instant::now() + state.timer.paused_for;
            state.timer.paused = false;
            state.timer.running = true;
            changed = true;
        }
        "reset" => {
            reset_timer(&mut state);
            changed = true;
        }
        _ => {}
    }

    if changed {
        state.timer.revision = state.timer.revision.wrapping_add(1);
        wake.notify_one();
        emit_update(&app, &state);
    }
    snapshot(&state)
}

#[tauri::command]
fn get_timer_state(shared: State<'_, Shared>) -> TimerUpdate {
    snapshot(&shared.0 .0.lock().unwrap())
}

#[tauri::command]
fn get_program_library(shared: State<'_, Shared>) -> ProgramLibrary {
    library_snapshot(&shared.0 .0.lock().unwrap())
}

#[tauri::command]
fn import_program(
    program: ProgramDefinition,
    source: String,
    shared: State<'_, Shared>,
    app: tauri::AppHandle,
) -> Result<ProgramLibrary, String> {
    let (lock, wake) = &*shared.0;
    let mut state = lock.lock().unwrap();
    if state
        .programs
        .iter()
        .any(|existing| existing.name.eq_ignore_ascii_case(program.name.trim()))
    {
        return Err(format!(
            "A program named \"{}\" already exists.",
            program.name
        ));
    }

    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let mut id = format!("program-{nanos}");
    let mut suffix = 1;
    while state.programs.iter().any(|existing| existing.id == id) {
        id = format!("program-{nanos}-{suffix}");
        suffix += 1;
    }

    let stored = StoredProgram {
        id: id.clone(),
        name: program.name.trim().into(),
        source,
        entries: program.entries,
    };
    validate_program(&stored)?;

    let previous_programs = state.programs.clone();
    let previous_selected = state.selected_id.clone();
    state.programs.push(stored);
    state.selected_id = id;
    reset_timer(&mut state);
    if let Err(error) = persist(&state) {
        state.programs = previous_programs;
        state.selected_id = previous_selected;
        reset_timer(&mut state);
        return Err(error);
    }
    wake.notify_one();
    emit_update(&app, &state);
    Ok(library_snapshot(&state))
}

#[tauri::command]
fn select_program(
    id: &str,
    shared: State<'_, Shared>,
    app: tauri::AppHandle,
) -> Result<ProgramLibrary, String> {
    let (lock, wake) = &*shared.0;
    let mut state = lock.lock().unwrap();
    if !state.programs.iter().any(|program| program.id == id) {
        return Err("The selected program no longer exists.".into());
    }
    if state.selected_id == id {
        return Ok(library_snapshot(&state));
    }

    let previous = state.selected_id.clone();
    state.selected_id = id.into();
    reset_timer(&mut state);
    if let Err(error) = persist(&state) {
        state.selected_id = previous;
        reset_timer(&mut state);
        return Err(error);
    }
    wake.notify_one();
    emit_update(&app, &state);
    Ok(library_snapshot(&state))
}

#[tauri::command]
fn delete_program(
    id: &str,
    shared: State<'_, Shared>,
    app: tauri::AppHandle,
) -> Result<ProgramLibrary, String> {
    let (lock, wake) = &*shared.0;
    let mut state = lock.lock().unwrap();
    if state.programs.len() == 1 {
        return Err("At least one timer program is required.".into());
    }
    let Some(index) = state.programs.iter().position(|program| program.id == id) else {
        return Err("The program no longer exists.".into());
    };

    let previous_programs = state.programs.clone();
    let previous_selected = state.selected_id.clone();
    let deleting_selected = state.selected_id == id;
    state.programs.remove(index);
    if deleting_selected {
        let next = index.min(state.programs.len() - 1);
        state.selected_id = state.programs[next].id.clone();
        reset_timer(&mut state);
    }
    if let Err(error) = persist(&state) {
        state.programs = previous_programs;
        state.selected_id = previous_selected;
        if deleting_selected {
            reset_timer(&mut state);
        }
        return Err(error);
    }
    if deleting_selected {
        wake.notify_one();
        emit_update(&app, &state);
    }
    Ok(library_snapshot(&state))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn activity(title: &str, duration: u64, color: &str) -> Activity {
        Activity {
            title: title.into(),
            duration,
            color: color.into(),
        }
    }

    fn program(entries: Vec<ProgramEntry>) -> StoredProgram {
        StoredProgram {
            id: "test".into(),
            name: "Test".into(),
            source: "Test: []".into(),
            entries,
        }
    }

    #[test]
    fn finite_blocks_repeat_exactly_before_an_explicit_ending() {
        let program = program(vec![
            ProgramEntry::Repeat {
                count: Some(3),
                activities: vec![activity("Work", 30, "red"), activity("Rest", 15, "green")],
            },
            ProgramEntry::Activity {
                title: "Work".into(),
                duration: 30,
                color: "red".into(),
            },
        ]);
        let mut cursor = Cursor::default();
        let mut titles = vec![current_activity_owned(&program, &cursor).unwrap().title];
        while let Some(next) = advance_cursor(&program, &mut cursor) {
            titles.push(next.title);
        }
        assert_eq!(
            titles,
            ["Work", "Rest", "Work", "Rest", "Work", "Rest", "Work"]
        );
    }

    #[test]
    fn forever_blocks_cycle_without_expansion() {
        let program = program(vec![ProgramEntry::Repeat {
            count: None,
            activities: vec![activity("Work", 30, "blue"), activity("Break", 10, "green")],
        }]);
        let mut cursor = Cursor::default();
        let mut titles = vec![current_activity_owned(&program, &cursor).unwrap().title];
        for _ in 0..5 {
            titles.push(advance_cursor(&program, &mut cursor).unwrap().title);
        }
        assert_eq!(titles, ["Work", "Break", "Work", "Break", "Work", "Break"]);
    }

    #[test]
    fn forever_must_be_the_final_entry() {
        let invalid = program(vec![
            ProgramEntry::Repeat {
                count: None,
                activities: vec![activity("Work", 30, "blue")],
            },
            ProgramEntry::Activity {
                title: "Unreachable".into(),
                duration: 10,
                color: "red".into(),
            },
        ]);
        assert_eq!(
            validate_program(&invalid).unwrap_err(),
            "The forever block must be the final program entry."
        );
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let shared = Arc::new((Mutex::new(initial_state()), Condvar::new()));
    let managed_state = Arc::clone(&shared);
    let worker_state = Arc::clone(&shared);

    tauri::Builder::default()
        .manage(Shared(managed_state))
        .invoke_handler(tauri::generate_handler![
            timer_action,
            get_timer_state,
            get_program_library,
            import_program,
            select_program,
            delete_program,
        ])
        .setup(move |app| {
            let app_data = app.path().app_data_dir()?;
            fs::create_dir_all(&app_data)?;
            let storage_path = app_data.join("programs.json");
            {
                let (lock, _) = &*worker_state;
                let mut state = lock.lock().unwrap();
                if let Some(library) = load_library(&storage_path) {
                    state.programs = library.programs;
                    state.selected_id = library.selected_id;
                }
                state.storage_path = Some(storage_path);
                reset_timer(&mut state);
            }

            let show = MenuItem::with_id(app, "show", "Show Timer", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show, &quit])?;
            TrayIconBuilder::new()
                .icon(app.default_window_icon().expect("application icon").clone())
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(move |app, event| match event.id.as_ref() {
                    "show" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.unminimize();
                            let _ = window.set_focus();
                        }
                    }
                    "quit" => app.exit(0),
                    _ => {}
                })
                .build(app)?;

            let handle = app.handle().clone();
            thread::spawn(move || loop {
                let (lock, wake) = &*worker_state;
                let mut state = lock.lock().unwrap();
                while !state.timer.running || state.timer.paused {
                    state = wake.wait(state).unwrap();
                }

                let revision = state.timer.revision;
                let wait_for = state
                    .timer
                    .deadline
                    .saturating_duration_since(Instant::now());
                let (next, result) = wake.wait_timeout(state, wait_for).unwrap();
                state = next;

                if result.timed_out()
                    && state.timer.running
                    && !state.timer.paused
                    && state.timer.revision == revision
                {
                    let program = selected_program(&state).clone();
                    let next_activity = advance_cursor(&program, &mut state.timer.cursor);
                    state.timer.revision = state.timer.revision.wrapping_add(1);
                    match next_activity {
                        Some(activity) => {
                            let duration = Duration::from_secs(activity.duration);
                            state.timer.deadline = Instant::now() + duration;
                            state.timer.paused_for = duration;
                        }
                        None => {
                            state.timer.running = false;
                            state.timer.paused = false;
                            state.timer.completed = true;
                            state.timer.paused_for = Duration::ZERO;
                        }
                    }
                    let update = snapshot(&state);
                    drop(state);
                    let _ = handle.emit("timer-alarm", ());
                    let _ = handle.emit("timer-update", update);
                }
            });
            Ok(())
        })
        .on_window_event(|window, event| match event {
            tauri::WindowEvent::CloseRequested { api, .. } => {
                api.prevent_close();
                let _ = window.hide();
            }
            tauri::WindowEvent::Resized(_) => {
                if window.is_minimized().unwrap_or(false) {
                    let _ = window.hide();
                }
            }
            _ => {}
        })
        .run(tauri::generate_context!())
        .expect("error while running application");
}
