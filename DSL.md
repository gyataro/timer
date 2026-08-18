# Timer DSL v1

Timer DSL is a small, human-readable language for programmable interval timers.
It uses YAML for ordering, indentation, comments, and optional reuse while keeping
the timer notation compact enough to read as a workout plan.

This document is the normative specification for version 1 of the language. The
key words **MUST**, **MUST NOT**, **SHOULD**, **SHOULD NOT**, and **MAY** describe
conformance requirements.

## Design principles

- One activity is readable on one line.
- A repeated block repeats its contents exactly.
- Repeated blocks are anonymous and cannot be nested.
- Special handling for the final repetition does not exist. Write a different
  ending explicitly after the repeated block.
- Programs use named Fluent color families instead of raw color values.
- YAML features are optional conveniences, not part of the timer execution model.

## Complete example

```yaml
Workout:
  - 10s Get ready: marigold

  - 3x:
      - 30s Work out: red
      - 15s Rest: green

  - 30s Work out: red

  - 2x:
      - 45s Stretch: blue
      - 15s Relax: lavender
      - 10s Change position: orange

  - 45s Stretch: blue
  - 15s Relax: lavender
  - 45s Cool down: lightBlue
```

The first repeated block produces this timeline:

```text
Work out, Rest, Work out, Rest, Work out, Rest, Work out
```

The final `Work out` is written explicitly. There is no "except on the last
repetition" modifier.

## YAML document

A timer program MUST be a UTF-8 YAML 1.2 document using the Core Schema.

The following YAML features are supported:

- block mappings and sequences;
- quoted and plain scalars;
- comments;
- anchors and aliases, subject to the restrictions below.

The following YAML features are not supported:

- multiple documents in one file;
- custom tags;
- duplicate mapping keys;
- the YAML merge key `<<`;
- cyclic aliases.

Tabs MUST NOT be used for indentation. Canonical formatting uses two spaces per
indentation level.

## Program

The document root MUST be a mapping with exactly one entry:

```yaml
Program name:
  - entry
  - entry
```

The mapping key is the program name. It MUST be a non-empty string after trimming
leading and trailing whitespace. The mapping value MUST be a non-empty sequence of
activities and repeated blocks.

Program names may contain Unicode text. Quote a name when YAML requires it:

```yaml
"Morning: mobility":
  - 5m Stretch: lavender
```

## Activity

An activity is a one-entry YAML mapping:

```yaml
- 30s Work out: red
```

Its mapping key contains a duration, one or more whitespace characters, and a
title. Its mapping value is a Fluent color name.

Conceptually:

```text
duration title: color
```

An activity MUST have exactly one mapping entry. The duration and title are split
at the first whitespace after the duration.

### Duration

A duration is one or more integer quantities followed by the units `h`, `m`, or
`s`:

```text
10s
5m
1h
1m30s
1h20m
1h20m30s
```

Units MUST appear at most once and in descending order: hours, minutes, seconds.
Unit matching is case-insensitive on input, but canonical output uses lowercase.
Whitespace is not permitted inside a duration.

Each quantity MUST be a non-negative decimal integer. At least one quantity MUST
be greater than zero. Leading zeroes are accepted on input but removed by canonical
formatting.

Quantities may exceed their usual clock ranges. A formatter normalizes the total
duration:

```text
90s       -> 1m30s
60m       -> 1h
1h90m     -> 2h30m
0h05m00s  -> 5m
```

The following durations are invalid:

```text
0s         # zero duration
1.5m       # fractions are not supported
1m 30s     # internal whitespace
30s1m      # units are out of order
1m2m       # duplicate unit
PT30S      # ISO 8601 is not Timer DSL notation
00:30      # clock notation is not Timer DSL notation
```

Implementations MUST calculate durations using integer arithmetic and MUST NOT
silently round values that exceed the host language's safe integer range.

### Title

The activity title MUST contain at least one non-whitespace character. Leading and
trailing whitespace is not part of the title. Internal whitespace is preserved.

Titles may contain Unicode text and YAML punctuation. Quote the complete activity
key when the title would otherwise change the YAML structure:

```yaml
- "30s Run: easy pace": green
```

The application displays the activity title in the window title while the activity
is active.

### Color

The activity value MUST be one of these Fluent global color family names:

```text
darkRed      burgundy     cranberry    red           darkOrange
bronze       pumpkin      orange       peach         marigold
yellow       gold         brass         brown         darkBrown
lime         forest       seafoam       lightGreen    green
darkGreen    lightTeal    teal          darkTeal      cyan
steel        lightBlue    blue          royalBlue     darkBlue
cornflower   navy         lavender      purple        darkPurple
orchid       grape        berry         lilac         pink
hotPink      magenta      plum          beige         mink
silver       platinum     anchor         charcoal
```

These names correspond to the color families in the
[Fluent global color palette](https://github.com/microsoft/fluentui/blob/master/packages/tokens/src/global/colors.ts).
Names are case-sensitive and MUST use the spelling shown above. Raw CSS colors,
hex values, RGB values, and Fluent shade names are invalid.

The name expresses a semantic color family, not a fixed RGB value. A renderer MUST
select an appropriate Fluent variant for the active light, dark, or high-contrast
theme. The bottom status bar and the paused icon button MUST resolve the same color
name through the same theme mapping so they remain visually consistent.

## Finite repeated block

A finite repeated block is a one-entry mapping whose key is a positive decimal
integer followed immediately by lowercase `x`:

```yaml
- 4x:
    - 30s Work out: red
    - 15s Rest: green
```

The block value MUST be a non-empty sequence containing activities only. Repeated
blocks MUST NOT contain other repeated blocks.

The repetition count MUST be at least one and MUST NOT contain a sign, decimal
point, separator, or leading zero. Therefore `1x` and `12x` are valid; `0x`, `01x`,
`+2x`, and `2X` are invalid.

There is no language-defined maximum finite count. Implementations MUST parse the
count as an exact integer without rounding. They MAY reject a program before
execution when a documented runtime resource limit would be exceeded.

The block executes every contained activity, in order, exactly the stated number
of times. There are no implicit rest intervals, separators, or final-iteration
exceptions.

`1x` is valid but equivalent to writing its activities directly. It can still be
useful when generating or editing a program.

## Infinite repeated block

The key `forever` repeats a non-empty sequence of activities without a final
iteration:

```yaml
20-20-20:
  - forever:
      - 20m Work: blue
      - 20s Break: green
```

An infinite block MUST be the final top-level entry because subsequent entries
could never execute. It MUST contain activities only and MUST NOT contain another
repeated block.

The word `forever` is lowercase and case-sensitive.

## Exact endings

To omit a rest or transition after the final work interval, repeat the complete
round one fewer time and write the final work interval explicitly:

```yaml
Four rounds:
  - 3x:
      - 30s Work: red
      - 15s Rest: green
  - 30s Work: red
```

This rule is deliberately generic. Any ending can differ without adding
conditional syntax to activities or repeated blocks:

```yaml
Intervals:
  - 4x:
      - 1m Run: red
      - 2m Walk: green
  - 5m Cool down: blue
```

## Anchors and aliases

YAML anchors and aliases MAY reuse complete activities or repeated-block bodies.
They do not create Timer DSL identifiers and do not change execution semantics.

Reuse a complete activity:

```yaml
Workout:
  - 3x:
      - &work
        30s Work out: red
      - 15s Rest: green
  - *work
```

Reuse a repeated-block body with a different count:

```yaml
Workout:
  - 3x: &round
      - 30s Work out: red
      - 15s Rest: green
  - 2x: *round
```

After resolving an alias, the resulting node MUST be valid at the alias location.
An alias cannot be used as a program name, duration, title, color, or repetition
count. Anchor names are serialization details and MUST NOT be exposed as timer
program or activity names.

Parsers MUST reject cyclic aliases and MUST place implementation limits on alias
count and expansion size. A parser SHOULD allow at least 100 alias references and
10,000 resolved YAML nodes. Aliased values SHOULD be normalized into independent
internal values so runtime mutation cannot affect another activity.

## Comments

YAML comments may appear anywhere YAML permits them:

```yaml
Desk routine:
  - 20m Focus: blue       # Disable notifications
  - 20s Look far away: green
```

Comments have no execution semantics. A formatter SHOULD preserve them when
possible.

## Execution model

After parsing and validation, a program is evaluated from the first top-level
entry to the last:

1. An activity sets the window title and active color, then counts down its full
   duration.
2. A finite repeated block evaluates its activity sequence exactly `n` times.
3. An infinite repeated block evaluates its activity sequence repeatedly until the
   user stops the program.
4. When a finite program reaches its end, it is complete.

Pausing freezes the current activity's remaining duration. Resuming continues the
same activity. Pausing, resuming, or restarting MUST NOT alter the expanded order
of activities.

Implementations SHOULD evaluate repeated blocks lazily. They MUST NOT expand a
large finite count or `forever` into an in-memory list before execution.

## Validation

A conforming parser MUST validate both YAML structure and Timer DSL semantics
before execution. It MUST NOT partially execute an invalid program.

Diagnostics SHOULD include:

- the source line and column;
- the invalid value;
- the rule that was violated;
- a suggested correction when one is clear.

Examples:

```text
Line 4, column 5: repetition count "0x" must be at least 1x.
Line 6, column 22: "gren" is not a Fluent color; did you mean "green"?
Line 9, column 7: repeated blocks cannot contain another repeated block.
Line 12, column 3: "forever" must be the final program entry.
```

Implementations SHOULD report multiple independent validation errors in one pass.

## Canonical formatting

A canonical formatter produces:

- one YAML document;
- two-space indentation and no tabs;
- block-style mappings and sequences;
- lowercase, normalized durations;
- lowercase `x` and `forever`;
- color names with the exact canonical casing listed above;
- no leading zeroes in repetition counts;
- quoted strings only where needed for valid, unambiguous YAML.

Blank lines may be inserted between logical groups for readability and have no
semantic effect.

## Informal grammar

YAML is parsed first. The following grammar describes the Timer DSL values within
the resulting YAML nodes; it is not a replacement for the YAML grammar.

```text
program       = { program-name: entries }
entries       = entry, { entry }
entry         = activity | finite-repeat | infinite-repeat
activity      = { activity-key: color }
activity-key  = duration, whitespace, title
finite-repeat = { positive-integer "x": activities }
infinite-repeat = { "forever": activities }
activities    = activity, { activity }
duration      = [hours], [minutes], [seconds]
hours         = integer, ("h" | "H")
minutes       = integer, ("m" | "M")
seconds       = integer, ("s" | "S")
```

At least one duration unit must be present, and the total duration must be greater
than zero. The structural and semantic rules in the preceding sections take
precedence over this informal grammar.

## Non-goals in version 1

Timer DSL v1 intentionally does not provide:

- nested repeated blocks;
- named blocks or block references;
- variables, expressions, or calculations;
- conditional activities or special final iterations;
- randomization;
- parallel activities;
- calendar scheduling;
- custom colors or theme definitions;
- audio, notification, or command configuration.

These features can be considered independently in later versions without making
the core timer notation more difficult to read.
