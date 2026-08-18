import { SwatchRGB } from "@fluentui/web-components";

const fluentPrimaryColors: Record<string, string> = {
  darkRed: "#750b1c", burgundy: "#a4262c", cranberry: "#c50f1f",
  red: "#d13438", darkOrange: "#da3b01", bronze: "#a74109",
  pumpkin: "#ca5010", orange: "#f7630c", peach: "#ff8c00",
  marigold: "#eaa300", yellow: "#fde300", gold: "#c19c00",
  brass: "#986f0b", brown: "#8e562e", darkBrown: "#4d291c",
  lime: "#73aa24", forest: "#498205", seafoam: "#00cc6a",
  lightGreen: "#13a10e", green: "#107c10", darkGreen: "#0b6a0b",
  lightTeal: "#00b7c3", teal: "#038387", darkTeal: "#006666",
  cyan: "#0099bc", steel: "#005b70", lightBlue: "#3a96dd",
  blue: "#0078d4", royalBlue: "#004e8c", darkBlue: "#003966",
  cornflower: "#4f6bed", navy: "#0027b4", lavender: "#7160e8",
  purple: "#5c2e91", darkPurple: "#401b6c", orchid: "#8764b8",
  grape: "#881798", berry: "#c239b3", lilac: "#b146c2",
  pink: "#e43ba6", hotPink: "#e3008c", magenta: "#bf0077",
  plum: "#77004d", beige: "#7a7574", mink: "#5d5a58",
  silver: "#859599", platinum: "#69797e", anchor: "#394146",
  charcoal: "#393939",
};

function swatch(hex: string): SwatchRGB {
  const value = Number.parseInt(hex.slice(1), 16);
  return SwatchRGB.create(
    ((value >> 16) & 0xff) / 255,
    ((value >> 8) & 0xff) / 255,
    (value & 0xff) / 255,
  );
}

const swatches = new Map(
  Object.entries(fluentPrimaryColors).map(([name, value]) => [name, swatch(value)]),
);

export function colorSwatch(name: string): SwatchRGB {
  return swatches.get(name) ?? swatches.get("blue")!;
}
