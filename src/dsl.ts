import { parseAllDocuments } from "yaml";
import type { Activity, ProgramDefinition, ProgramEntry } from "./types";

const MAX_SOURCE_BYTES = 1024 * 1024;
const MAX_SAFE_VALUE = BigInt(Number.MAX_SAFE_INTEGER);

export const fluentColorNames = [
  "darkRed", "burgundy", "cranberry", "red", "darkOrange",
  "bronze", "pumpkin", "orange", "peach", "marigold",
  "yellow", "gold", "brass", "brown", "darkBrown",
  "lime", "forest", "seafoam", "lightGreen", "green",
  "darkGreen", "lightTeal", "teal", "darkTeal", "cyan",
  "steel", "lightBlue", "blue", "royalBlue", "darkBlue",
  "cornflower", "navy", "lavender", "purple", "darkPurple",
  "orchid", "grape", "berry", "lilac", "pink",
  "hotPink", "magenta", "plum", "beige", "mink",
  "silver", "platinum", "anchor", "charcoal",
] as const;

const fluentColors = new Set<string>(fluentColorNames);
const durationPattern = /^(?:(\d+)[hH])?(?:(\d+)[mM])?(?:(\d+)[sS])?$/;
const repeatPattern = /^([1-9]\d*)x$/;

export class DslError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "DslError";
  }
}

function mappingEntries(value: unknown, context: string): [unknown, unknown][] {
  if (value instanceof Map) return [...value.entries()];
  if (value === null || typeof value !== "object" || Array.isArray(value)) {
    throw new DslError(`${context} must be a YAML mapping.`);
  }
  return Object.entries(value as Record<string, unknown>);
}

function singleEntry(
  value: unknown,
  context: string,
): [string, unknown] {
  const entries = mappingEntries(value, context);
  if (entries.length !== 1) {
    throw new DslError(`${context} must contain exactly one mapping entry.`);
  }
  const [key, body] = entries[0];
  if (typeof key !== "string") {
    throw new DslError(`${context} mapping key must be text.`);
  }
  return [key, body];
}

function parseDuration(source: string, context: string): number {
  const match = durationPattern.exec(source);
  if (!match || match.slice(1).every((part) => part === undefined)) {
    throw new DslError(`${context} has invalid duration "${source}".`);
  }

  const hours = BigInt(match[1] ?? 0);
  const minutes = BigInt(match[2] ?? 0);
  const seconds = BigInt(match[3] ?? 0);
  const total = hours * 3600n + minutes * 60n + seconds;
  if (total === 0n) {
    throw new DslError(`${context} must have a duration greater than zero.`);
  }
  if (total > MAX_SAFE_VALUE) {
    throw new DslError(`${context} exceeds this application's maximum duration.`);
  }
  return Number(total);
}

function parseActivity(value: unknown, context: string): Activity {
  const [activityKey, color] = singleEntry(value, context);
  const separator = activityKey.search(/\s/);
  if (separator <= 0) {
    throw new DslError(`${context} must use "duration title: color".`);
  }

  const durationText = activityKey.slice(0, separator);
  const title = activityKey.slice(separator).trim();
  if (!title) {
    throw new DslError(`${context} must have a title.`);
  }
  if (typeof color !== "string" || !fluentColors.has(color)) {
    throw new DslError(
      `${context} uses unsupported Fluent color "${String(color)}".`,
    );
  }

  return {
    title,
    duration: parseDuration(durationText, context),
    color,
  };
}

function parseEntry(value: unknown, index: number): ProgramEntry {
  const context = `Entry ${index + 1}`;
  const [key, body] = singleEntry(value, context);
  const repeat = repeatPattern.exec(key);

  if (key === "forever" || repeat) {
    if (!Array.isArray(body) || body.length === 0) {
      throw new DslError(`${context} repeated block must contain activities.`);
    }
    const count = repeat ? BigInt(repeat[1]) : null;
    if (count !== null && count > MAX_SAFE_VALUE) {
      throw new DslError(`${context} repetition count is too large for this application.`);
    }
    return {
      type: "repeat",
      count: count === null ? null : Number(count),
      activities: body.map((activity, activityIndex) =>
        parseActivity(activity, `${context}, activity ${activityIndex + 1}`),
      ),
    };
  }

  return { type: "activity", ...parseActivity(value, context) };
}

export function parseProgram(source: string): ProgramDefinition {
  if (!source.trim()) throw new DslError("The selected file is empty.");
  if (new TextEncoder().encode(source).length > MAX_SOURCE_BYTES) {
    throw new DslError("Timer programs must be no larger than 1 MiB.");
  }

  const documents = parseAllDocuments(source, {
    schema: "core",
    uniqueKeys: true,
  });
  if (documents.length !== 1) {
    throw new DslError("A timer file must contain exactly one YAML document.");
  }

  const document = documents[0];
  if (document.errors.length > 0) {
    throw new DslError(document.errors[0].message);
  }
  if (document.warnings.length > 0) {
    throw new DslError(document.warnings[0].message);
  }

  let root: unknown;
  try {
    root = document.toJS({ mapAsMap: true, maxAliasCount: 100 });
  } catch (error) {
    throw new DslError(error instanceof Error ? error.message : String(error));
  }

  const [name, body] = singleEntry(root, "Program");
  if (!name.trim()) throw new DslError("The program name cannot be empty.");
  if (!Array.isArray(body) || body.length === 0) {
    throw new DslError("A program must contain at least one entry.");
  }

  const entries = body.map(parseEntry);
  const foreverIndex = entries.findIndex(
    (entry) => entry.type === "repeat" && entry.count === null,
  );
  if (foreverIndex >= 0 && foreverIndex !== entries.length - 1) {
    throw new DslError('The "forever" block must be the final program entry.');
  }

  return { name: name.trim(), entries };
}
