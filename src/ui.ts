import type { ProgramLibrary, TimerUpdate } from "./types";

const element = <T extends HTMLElement>(id: string): T =>
  document.querySelector<T>(`#${id}`)!;

type UiHandlers = {
  timerAction: (name: string) => void | Promise<void>;
  openPrograms: () => void | Promise<void>;
  closePrograms: () => void | Promise<void>;
  importProgram: (file: File) => void | Promise<void>;
  selectProgram: (id: string) => void | Promise<void>;
  deleteProgram: (id: string) => void | Promise<void>;
};

let handlers: UiHandlers | undefined;
let pendingDelete: { id: string; name: string } | undefined;

export function renderTimer(state: TimerUpdate): void {
  const underOneMinute = state.remaining < 60;
  const value = underOneMinute
    ? state.remaining.toString()
    : Math.floor(state.remaining / 60).toString();

  element<HTMLDivElement>("activity-name").textContent = state.activityName;
  element<HTMLSpanElement>("timer-value").textContent = value;
  element<HTMLSpanElement>("timer-unit").textContent = underOneMinute ? "sec" : "mins";
  element("play").hidden = state.running || state.paused;
  element("pause").hidden = !state.running || state.paused;
  element("resume").hidden = !state.paused;
  element("play").title = state.completed ? "Restart timer" : "Start timer";
  element("play").setAttribute(
    "aria-label",
    state.completed ? "Restart timer" : "Start timer",
  );
}

function icon(path: string): SVGSVGElement {
  const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
  svg.setAttribute("viewBox", "0 0 24 24");
  svg.setAttribute("width", "18");
  svg.setAttribute("height", "18");
  svg.setAttribute("fill", "currentColor");
  svg.setAttribute("aria-hidden", "true");
  const shape = document.createElementNS("http://www.w3.org/2000/svg", "path");
  shape.setAttribute("d", path);
  svg.append(shape);
  return svg;
}

export function renderProgramLibrary(library: ProgramLibrary): void {
  const list = element<HTMLDivElement>("program-list");
  list.replaceChildren();

  for (const program of library.programs) {
    const row = document.createElement("div");
    row.className = "program-row";
    row.setAttribute("role", "listitem");
    row.dataset.selected = program.selected.toString();

    const select = document.createElement("fluent-button");
    select.className = "program-select";
    select.setAttribute("appearance", "stealth");
    select.textContent = program.name;
    select.title = program.selected ? `${program.name} is active` : `Use ${program.name}`;
    if (program.selected) {
      select.setAttribute("aria-current", "true");
    } else {
      select.onclick = () => void handlers?.selectProgram(program.id);
    }

    row.append(select);

    const remove = document.createElement("fluent-button");
    remove.className = "program-delete";
    remove.setAttribute("appearance", "stealth");
    remove.setAttribute("aria-label", `Delete ${program.name}`);
    remove.title = `Delete ${program.name}`;
    remove.append(icon("M6 7h12l-1 14H7L6 7zm3-3h6l1 2H8l1-2zm1 6v8h2v-8h-2zm4 0v8h2v-8h-2z"));
    if (library.programs.length === 1) {
      remove.setAttribute("disabled", "");
      remove.title = "At least one program is required";
    } else {
      remove.onclick = () => {
        pendingDelete = { id: program.id, name: program.name };
        element<HTMLParagraphElement>("delete-dialog-description").textContent =
          `“${program.name}” will be removed from Timer.`;
        element("delete-dialog").hidden = false;
        element("cancel-delete").focus();
      };
    }
    row.append(remove);
    list.append(row);
  }
}

export function showProgramMessage(message = "", error = false): void {
  const status = element<HTMLParagraphElement>("program-message");
  status.textContent = message;
  status.hidden = !message;
  status.dataset.error = error.toString();
  status.setAttribute("role", error ? "alert" : "status");
}

export function showProgramsPage(show: boolean): void {
  element("timer-view").hidden = show;
  element("programs-view").hidden = !show;
  if (show) element("close-programs").focus();
}

export function setUiHandlers(next: UiHandlers): void {
  handlers = next;
  element("play").onclick = () => void next.timerAction("play");
  element("pause").onclick = () => void next.timerAction("pause");
  element("resume").onclick = () => void next.timerAction("resume");
  element("reset").onclick = () => void next.timerAction("reset");
  element("open-programs").onclick = () => void next.openPrograms();
  element("close-programs").onclick = () => void next.closePrograms();

  const fileInput = element<HTMLInputElement>("program-file");
  element("upload-program").onclick = () => fileInput.click();
  fileInput.onchange = () => {
    const file = fileInput.files?.[0];
    fileInput.value = "";
    if (file) void next.importProgram(file);
  };

  const closeDeleteDialog = (): void => {
    element("delete-dialog").hidden = true;
    pendingDelete = undefined;
  };
  element("cancel-delete").onclick = closeDeleteDialog;
  element("confirm-delete").onclick = () => {
    const program = pendingDelete;
    closeDeleteDialog();
    if (program) void next.deleteProgram(program.id);
  };
  element("delete-dialog").addEventListener("dismiss", closeDeleteDialog);
}
