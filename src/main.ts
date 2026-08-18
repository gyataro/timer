import { playAlarm } from "./alarm";
import { colorSwatch } from "./colors";
import { parseProgram } from "./dsl";
import {
  renderProgramLibrary,
  renderTimer,
  setUiHandlers,
  showProgramMessage,
  showProgramsPage,
} from "./ui";
import type { ProgramLibrary, TimerUpdate } from "./types";
import {
  accentBaseColor,
  baseLayerLuminance,
  fluentBadge,
  fluentButton,
  fluentCard,
  fluentDivider,
  provideFluentDesignSystem,
  StandardLuminance,
} from "@fluentui/web-components";
import { invoke } from "@tauri-apps/api/core";
import { LogicalSize } from "@tauri-apps/api/dpi";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";

provideFluentDesignSystem().register(
  fluentBadge(),
  fluentButton(),
  fluentCard(),
  fluentDivider(),
);

const systemTheme = window.matchMedia("(prefers-color-scheme: dark)");

function applySystemTheme(): void {
  document.documentElement.style.colorScheme = systemTheme.matches ? "dark" : "light";
  baseLayerLuminance.setValueFor(
    document.body,
    systemTheme.matches ? StandardLuminance.DarkMode : StandardLuminance.LightMode,
  );
}

applySystemTheme();
systemTheme.addEventListener("change", applySystemTheme);

let state: TimerUpdate = {
  programId: "",
  programName: "20-20-20",
  activityName: "Work",
  color: "blue",
  remaining: 20 * 60,
  paused: false,
  running: false,
  completed: false,
};

let secondsTick: number | undefined;
let actionPending = false;
let libraryPending = false;

function scheduleSecondsTick(): void {
  if (secondsTick !== undefined) window.clearTimeout(secondsTick);
  if (!state.running || state.paused || state.remaining === 0) return;

  secondsTick = window.setTimeout(() => {
    state = { ...state, remaining: Math.max(0, state.remaining - 1) };
    renderTimer(state);
    scheduleSecondsTick();
  }, 1000);
}

function applyUpdate(update: TimerUpdate): void {
  state = update;
  accentBaseColor.setValueFor(document.body, colorSwatch(update.color));
  renderTimer(state);
  scheduleSecondsTick();
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

async function resizeForPrograms(show: boolean): Promise<void> {
  const size = show ? new LogicalSize(390, 430) : new LogicalSize(200, 180);
  await getCurrentWindow().setSize(size);
}

async function openPrograms(): Promise<void> {
  if (libraryPending) return;
  libraryPending = true;
  try {
    const library = await invoke<ProgramLibrary>("get_program_library");
    renderProgramLibrary(library);
    showProgramMessage();
    showProgramsPage(true);
    await resizeForPrograms(true);
  } catch (error) {
    showProgramMessage(errorMessage(error), true);
  } finally {
    libraryPending = false;
  }
}

async function closePrograms(): Promise<void> {
  showProgramsPage(false);
  await resizeForPrograms(false);
}

async function importProgram(file: File): Promise<void> {
  if (libraryPending) return;
  libraryPending = true;
  showProgramMessage(`Importing ${file.name}…`);
  try {
    const source = await file.text();
    const program = parseProgram(source);
    const library = await invoke<ProgramLibrary>("import_program", { program, source });
    renderProgramLibrary(library);
    showProgramMessage(`Imported ${program.name}.`);
  } catch (error) {
    showProgramMessage(errorMessage(error), true);
  } finally {
    libraryPending = false;
  }
}

async function selectProgram(id: string): Promise<void> {
  if (libraryPending) return;
  libraryPending = true;
  try {
    const library = await invoke<ProgramLibrary>("select_program", { id });
    renderProgramLibrary(library);
    showProgramMessage();
  } catch (error) {
    showProgramMessage(errorMessage(error), true);
  } finally {
    libraryPending = false;
  }
}

async function deleteProgram(id: string): Promise<void> {
  if (libraryPending) return;
  libraryPending = true;
  try {
    const library = await invoke<ProgramLibrary>("delete_program", { id });
    renderProgramLibrary(library);
    showProgramMessage();
  } catch (error) {
    showProgramMessage(errorMessage(error), true);
  } finally {
    libraryPending = false;
  }
}

async function connectTauri(): Promise<void> {
  const timerAction = async (name: string): Promise<void> => {
    if (actionPending) return;
    actionPending = true;
    try {
      applyUpdate(await invoke<TimerUpdate>("timer_action", { action: name }));
    } finally {
      actionPending = false;
    }
  };

  const unlistenUpdate = await listen<TimerUpdate>("timer-update", (event) =>
    applyUpdate(event.payload));
  const unlistenAlarm = await listen("timer-alarm", playAlarm);
  window.addEventListener("beforeunload", () => {
    void unlistenUpdate();
    void unlistenAlarm();
  }, { once: true });

  setUiHandlers({
    timerAction,
    openPrograms,
    closePrograms,
    importProgram,
    selectProgram,
    deleteProgram,
  });
  applyUpdate(await invoke<TimerUpdate>("get_timer_state"));
}

renderTimer(state);
void connectTauri();
