export type Activity = {
  title: string;
  duration: number;
  color: string;
};

export type ProgramEntry =
  | ({ type: "activity" } & Activity)
  | {
      type: "repeat";
      count: number;
      activities: Activity[];
    };

export type ProgramDefinition = {
  name: string;
  repeat: boolean;
  entries: ProgramEntry[];
};

export type ProgramSummary = {
  id: string;
  name: string;
  selected: boolean;
};

export type ProgramLibrary = {
  selectedId: string;
  programs: ProgramSummary[];
};

export type ProgramExport = {
  name: string;
  source: string;
};

export type TimerUpdate = {
  programId: string;
  programName: string;
  activityName: string;
  color: string;
  remaining: number;
  paused: boolean;
  running: boolean;
  canGoPrev: boolean;
  canGoNext: boolean;
};
