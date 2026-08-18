import type { ProgramLibrary, TimerUpdate } from "./types";

const element = <T extends HTMLElement>(id: string): T =>
  document.querySelector<T>(`#${id}`)!;

type UiHandlers = {
  timerAction: (name: string) => void | Promise<void>;
  openPrograms: () => void | Promise<void>;
  closePrograms: () => void | Promise<void>;
  importProgram: (file: File) => void | Promise<void>;
  selectProgram: (id: string) => void | Promise<void>;
  exportProgram: (id: string) => void | Promise<void>;
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

function closeProgramMenus(): void {
  document.querySelectorAll<HTMLElement>(".program-menu").forEach((menu) => {
    menu.hidden = true;
  });
  document.querySelectorAll<HTMLElement>(".program-menu-button").forEach((button) => {
    button.setAttribute("aria-expanded", "false");
  });
}

export function renderProgramLibrary(library: ProgramLibrary): void {
  const list = element<HTMLDivElement>("program-list");
  list.replaceChildren();

  for (const program of library.programs) {
    const row = document.createElement("div");
    row.className = "program-row";
    row.setAttribute("role", "listitem");
    row.dataset.selected = program.selected.toString();

    const select = document.createElement("button");
    select.className = "program-select";
    select.type = "button";
    select.title = program.selected ? `${program.name} is active` : `Use ${program.name}`;
    const name = document.createElement("span");
    name.className = "program-name";
    name.textContent = program.name;
    const detail = document.createElement("span");
    detail.className = "program-detail";
    detail.textContent = program.selected ? "Current program" : "Timer program";
    select.append(name, detail);
    if (program.selected) {
      select.setAttribute("aria-current", "true");
    } else {
      select.onclick = () => {
        closeProgramMenus();
        void handlers?.selectProgram(program.id);
      };
    }

    row.append(select);

    const menuContainer = document.createElement("div");
    menuContainer.className = "program-menu-container";
    const menuButton = document.createElement("fluent-button");
    menuButton.className = "program-menu-button";
    menuButton.setAttribute("appearance", "stealth");
    menuButton.setAttribute("aria-label", `More options for ${program.name}`);
    menuButton.setAttribute("aria-haspopup", "menu");
    menuButton.setAttribute("aria-expanded", "false");
    menuButton.title = "More options";
    menuButton.append(icon("M5 10a2 2 0 1 0 0 4 2 2 0 0 0 0-4zm7 0a2 2 0 1 0 0 4 2 2 0 0 0 0-4zm7 0a2 2 0 1 0 0 4 2 2 0 0 0 0-4z"));

    const menu = document.createElement("fluent-menu");
    menu.className = "program-menu";
    menu.hidden = true;
    const exportItem = document.createElement("fluent-menu-item");
    exportItem.textContent = "Export";
    exportItem.onclick = (event) => {
      event.stopPropagation();
      closeProgramMenus();
      void handlers?.exportProgram(program.id);
    };
    const deleteItem = document.createElement("fluent-menu-item");
    deleteItem.textContent = "Delete";
    if (library.programs.length === 1) {
      deleteItem.setAttribute("disabled", "");
      deleteItem.title = "At least one program is required";
    } else {
      deleteItem.onclick = (event) => {
        event.stopPropagation();
        closeProgramMenus();
        pendingDelete = { id: program.id, name: program.name };
        element<HTMLParagraphElement>("delete-dialog-description").textContent =
          `“${program.name}” will be removed from Timer.`;
        element("delete-dialog").hidden = false;
        element("cancel-delete").focus();
      };
    }
    menu.append(exportItem, deleteItem);
    menuButton.onclick = (event) => {
      event.stopPropagation();
      const opening = menu.hidden;
      closeProgramMenus();
      menu.hidden = !opening;
      menuButton.setAttribute("aria-expanded", opening.toString());
    };
    menuContainer.append(menuButton, menu);
    row.append(menuContainer);
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
  document.addEventListener("click", closeProgramMenus);
}
