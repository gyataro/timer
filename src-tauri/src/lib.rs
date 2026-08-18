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
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager, State,
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
        count: u64,
        activities: Vec<Activity>,
    },
}

#[derive(Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProgramDefinition {
    name: String,
    repeat: bool,
    entries: Vec<ProgramEntry>,
}

#[derive(Clone, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct StoredProgram {
    id: String,
    name: String,
    source: String,
    repeat: bool,
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

#[derive(serde::Serialize)]
struct ProgramExport {
    name: String,
    source: String,
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
    can_go_prev: bool,
    can_go_next: bool,
}

#[derive(Clone, Copy, Default, Debug, PartialEq)]
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
            "  repeat: true\n",
            "  activities:\n",
            "    - 20m Work: blue\n",
            "    - 20s Break: green\n",
        )
        .into(),
        repeat: true,
        entries: vec![
            ProgramEntry::Activity {
                title: "Work".into(),
                duration: 20 * 60,
                color: "blue".into(),
            },
            ProgramEntry::Activity {
                title: "Break".into(),
                duration: 20,
                color: "green".into(),
            },
        ],
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
                if cursor.repetition + 1 < *count {
                    cursor.repetition += 1;
                } else {
                    cursor.entry += 1;
                    cursor.repetition = 0;
                }
            }
        }
    }
    current_activity_owned(program, cursor)
}

/// The (repetition, activity) cursor position of an entry's last step.
fn last_step_within(entry: &ProgramEntry) -> (u64, usize) {
    match entry {
        ProgramEntry::Activity { .. } => (0, 0),
        ProgramEntry::Repeat { count, activities } => (count - 1, activities.len() - 1),
    }
}

/// Moves the cursor to the previous activity, wrapping to the program's
/// last activity when already at the first one. Unlike `advance_cursor`,
/// this is pure navigation: it never changes whether the timer is running.
fn retreat_cursor(program: &StoredProgram, cursor: &mut Cursor) -> Activity {
    if cursor.activity > 0 {
        cursor.activity -= 1;
    } else if cursor.repetition > 0 {
        cursor.repetition -= 1;
        cursor.activity = last_step_within(&program.entries[cursor.entry]).1;
    } else {
        cursor.entry = if cursor.entry > 0 {
            cursor.entry - 1
        } else {
            program.entries.len() - 1
        };
        let (repetition, activity) = last_step_within(&program.entries[cursor.entry]);
        cursor.repetition = repetition;
        cursor.activity = activity;
    }
    current_activity_owned(program, cursor).expect("validated program has a first activity")
}

/// Whether "prev" should be allowed: always for a repeating program, or
/// only past the first activity for a non-repeating one.
fn can_go_prev(program: &StoredProgram, cursor: &Cursor) -> bool {
    program.repeat || *cursor != Cursor::default()
}

/// Whether "next" should be allowed: always for a repeating program, or
/// only before the last activity for a non-repeating one.
fn can_go_next(program: &StoredProgram, cursor: &Cursor) -> bool {
    if program.repeat {
        return true;
    }
    let mut probe = *cursor;
    advance_cursor(program, &mut probe).is_some()
}

/// Called when `advance_cursor` has run through every top-level entry.
/// Resets the cursor to the first activity and reports whether the timer
/// should keep running (the program repeats) or stop there.
fn wrap_cursor(program: &StoredProgram, cursor: &mut Cursor) -> (Activity, bool) {
    *cursor = Cursor::default();
    let activity =
        current_activity_owned(program, cursor).expect("validated program has a first activity");
    (activity, program.repeat)
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
    let activity = current_activity_owned(program, &state.timer.cursor)
        .expect("validated cursor points to an activity");
    let remaining = if state.timer.paused {
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
        can_go_prev: can_go_prev(program, &state.timer.cursor),
        can_go_next: can_go_next(program, &state.timer.cursor),
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

    for entry in &program.entries {
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
                if *count == 0 {
                    return Err("A repetition count must be at least 1x.".into());
                }
                if activities.is_empty() {
                    return Err("A repeated block must contain an activity.".into());
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
        "next" if can_go_next(selected_program(&state), &state.timer.cursor) => {
            let program = selected_program(&state).clone();
            // The guard above only lets execution reach the end of the
            // program (advance_cursor returning None) when it repeats, so
            // wrap_cursor always reports keep_running here.
            let activity = match advance_cursor(&program, &mut state.timer.cursor) {
                Some(activity) => activity,
                None => wrap_cursor(&program, &mut state.timer.cursor).0,
            };
            let duration = Duration::from_secs(activity.duration);
            state.timer.deadline = Instant::now() + duration;
            state.timer.paused_for = duration;
            changed = true;
        }
        "prev" if can_go_prev(selected_program(&state), &state.timer.cursor) => {
            let program = selected_program(&state).clone();
            let activity = retreat_cursor(&program, &mut state.timer.cursor);
            let duration = Duration::from_secs(activity.duration);
            state.timer.deadline = Instant::now() + duration;
            state.timer.paused_for = duration;
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
fn get_program_source(id: &str, shared: State<'_, Shared>) -> Result<ProgramExport, String> {
    let state = shared.0 .0.lock().unwrap();
    let program = state
        .programs
        .iter()
        .find(|program| program.id == id)
        .ok_or_else(|| "The program no longer exists.".to_string())?;
    Ok(ProgramExport {
        name: program.name.clone(),
        source: program.source.clone(),
    })
}

#[tauri::command]
fn export_program_to_path(id: &str, path: &str, shared: State<'_, Shared>) -> Result<(), String> {
    let state = shared.0 .0.lock().unwrap();
    let program = state
        .programs
        .iter()
        .find(|program| program.id == id)
        .ok_or_else(|| "The program no longer exists.".to_string())?;
    fs::write(path, &program.source).map_err(|error| format!("Could not save program: {error}"))
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
        repeat: program.repeat,
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

    fn program(repeat: bool, entries: Vec<ProgramEntry>) -> StoredProgram {
        StoredProgram {
            id: "test".into(),
            name: "Test".into(),
            source: "Test: []".into(),
            repeat,
            entries,
        }
    }

    #[test]
    fn finite_blocks_repeat_exactly_before_an_explicit_ending() {
        let program = program(
            false,
            vec![
                ProgramEntry::Repeat {
                    count: 3,
                    activities: vec![activity("Work", 30, "red"), activity("Rest", 15, "green")],
                },
                ProgramEntry::Activity {
                    title: "Work".into(),
                    duration: 30,
                    color: "red".into(),
                },
            ],
        );
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
    fn repeating_program_wraps_back_to_the_first_activity_and_keeps_running() {
        let program = program(
            true,
            vec![
                activity_entry("Work", 30, "blue"),
                activity_entry("Break", 10, "green"),
            ],
        );
        let mut cursor = Cursor::default();
        advance_cursor(&program, &mut cursor); // Work -> Break
        assert!(advance_cursor(&program, &mut cursor).is_none()); // Break -> end of entries

        let (activity, keep_running) = wrap_cursor(&program, &mut cursor);
        assert_eq!(activity.title, "Work");
        assert!(keep_running);
        assert_eq!(cursor.entry, 0);
    }

    #[test]
    fn non_repeating_program_wraps_back_to_the_first_activity_and_stops() {
        let program = program(false, vec![activity_entry("Work", 30, "blue")]);
        let mut cursor = Cursor::default();
        assert!(advance_cursor(&program, &mut cursor).is_none());

        let (activity, keep_running) = wrap_cursor(&program, &mut cursor);
        assert_eq!(activity.title, "Work");
        assert!(!keep_running);
        assert_eq!(cursor.entry, 0);
    }

    #[test]
    fn retreat_cursor_reverses_advance_cursor_through_a_repeat_block() {
        let program = program(
            false,
            vec![
                activity_entry("A", 30, "red"),
                ProgramEntry::Repeat {
                    count: 2,
                    activities: vec![activity("X", 10, "blue"), activity("Y", 10, "green")],
                },
                activity_entry("B", 30, "orange"),
            ],
        );

        // Walk forward to the end, recording every activity title.
        // advance_cursor invalidates the cursor on the step that returns
        // None, so the last valid position has to be saved beforehand.
        let mut cursor = Cursor::default();
        let mut titles = vec![current_activity_owned(&program, &cursor).unwrap().title];
        loop {
            let before = cursor;
            match advance_cursor(&program, &mut cursor) {
                Some(activity) => titles.push(activity.title),
                None => {
                    cursor = before;
                    break;
                }
            }
        }
        assert_eq!(titles, ["A", "X", "Y", "X", "Y", "B"]);

        // Walking backward from the last activity should retrace the same
        // path in reverse.
        let mut reversed = vec![current_activity_owned(&program, &cursor).unwrap().title];
        for _ in 0..titles.len() - 1 {
            reversed.push(retreat_cursor(&program, &mut cursor).title);
        }
        titles.reverse();
        assert_eq!(reversed, titles);
        assert_eq!(cursor, Cursor::default());
    }

    #[test]
    fn retreat_cursor_wraps_to_the_last_activity_at_the_start() {
        let program = program(
            false,
            vec![
                activity_entry("A", 30, "red"),
                ProgramEntry::Repeat {
                    count: 2,
                    activities: vec![activity("X", 10, "blue")],
                },
            ],
        );
        let mut cursor = Cursor::default();
        let activity = retreat_cursor(&program, &mut cursor);
        assert_eq!(activity.title, "X");
        assert_eq!(cursor.entry, 1);
        assert_eq!(cursor.repetition, 1);
        assert_eq!(cursor.activity, 0);
    }

    #[test]
    fn prev_and_next_are_disabled_at_the_edges_of_a_non_repeating_program() {
        let program = program(
            false,
            vec![
                activity_entry("A", 30, "red"),
                activity_entry("B", 30, "blue"),
            ],
        );
        let mut cursor = Cursor::default();
        assert!(!can_go_prev(&program, &cursor));
        assert!(can_go_next(&program, &cursor));

        advance_cursor(&program, &mut cursor);
        assert!(can_go_prev(&program, &cursor));
        assert!(!can_go_next(&program, &cursor));
    }

    #[test]
    fn prev_and_next_are_always_enabled_for_a_repeating_program() {
        let program = program(true, vec![activity_entry("A", 30, "red")]);
        let cursor = Cursor::default();
        assert!(can_go_prev(&program, &cursor));
        assert!(can_go_next(&program, &cursor));
    }

    fn activity_entry(title: &str, duration: u64, color: &str) -> ProgramEntry {
        ProgramEntry::Activity {
            title: title.into(),
            duration,
            color: color.into(),
        }
    }
}

fn show_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let shared = Arc::new((Mutex::new(initial_state()), Condvar::new()));
    let managed_state = Arc::clone(&shared);
    let worker_state = Arc::clone(&shared);

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(Shared(managed_state))
        .invoke_handler(tauri::generate_handler![
            timer_action,
            get_timer_state,
            get_program_library,
            get_program_source,
            export_program_to_path,
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
                    "show" => show_main_window(app),
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
                        show_main_window(tray.app_handle());
                    }
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
                            let (activity, keep_running) =
                                wrap_cursor(&program, &mut state.timer.cursor);
                            let duration = Duration::from_secs(activity.duration);
                            state.timer.paused_for = duration;
                            if keep_running {
                                state.timer.deadline = Instant::now() + duration;
                            } else {
                                state.timer.running = false;
                                state.timer.paused = false;
                            }
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
