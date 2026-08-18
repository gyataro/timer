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
  element<HTMLButtonElement>("prev").disabled = !state.canGoPrev;
  element<HTMLButtonElement>("next").disabled = !state.canGoNext;
}

function icon(path: string): SVGSVGElement {
  const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
  svg.setAttribute("viewBox", "0 0 20 20");
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
    select.title = program.selected ? `${program.name} is selected` : `Use ${program.name}`;
    const name = document.createElement("span");
    name.className = "program-name";
    name.textContent = program.name;
    select.append(name);
    if (program.selected) {
      const detail = document.createElement("span");
      detail.className = "program-detail";
      detail.textContent = "Selected";
      select.append(detail);
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
    if (program.selected) {
      const selectedIcon = icon("M3.37 10.17a.5.5 0 0 0-.74.66l4 4.5c.19.22.52.23.72.02l10.5-10.5a.5.5 0 0 0-.7-.7L7.02 14.27z");
      selectedIcon.classList.add("program-selected-icon");
      selectedIcon.setAttribute("aria-hidden", "true");
      menuContainer.append(selectedIcon);
    }
    const menuButton = document.createElement("fluent-button");
    menuButton.className = "program-menu-button";
    menuButton.setAttribute("appearance", "stealth");
    menuButton.setAttribute("aria-label", `More options for ${program.name}`);
    menuButton.setAttribute("aria-haspopup", "menu");
    menuButton.setAttribute("aria-expanded", "false");
    menuButton.title = "More options";
    menuButton.append(icon("M6.25 10a1.25 1.25 0 1 1-2.5 0 1.25 1.25 0 0 1 2.5 0m5 0a1.25 1.25 0 1 1-2.5 0 1.25 1.25 0 0 1 2.5 0M15 11.25a1.25 1.25 0 1 0 0-2.5 1.25 1.25 0 0 0 0 2.5"));

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
  element("prev").onclick = () => void next.timerAction("prev");
  element("next").onclick = () => void next.timerAction("next");
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
