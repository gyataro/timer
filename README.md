# Timer

A minimal, programmable interval timer for the desktop — inspired by
[jotaen/timer](https://github.com/jotaen/timer), rebuilt as a native app because
one more browser tab was one too many. It follows Windows 11's native UI
conventions (Segoe UI Variable, Fluent controls, adaptive light/dark theming) so
it feels like part of the OS instead of a website pretending to be an app, and
it keeps running in the system tray when its window is closed.

Built with [Tauri 2](https://v2.tauri.app/), TypeScript, Vite, and [Fluent UI Web Components](https://github.com/microsoft/fluentui/tree/master/packages/web-components).

## Getting started

Timer programs are small, readable YAML files — no build step, no account, just
a text file describing a sequence of activities. The included 20-20-20 eye-care
routine works out of the box:

```yaml
20-20-20:
  repeat: true
  activities:
    - 20m Work: blue
    - 20s Break: green
```

Open the program list from the timer window to import a `.yaml`/`.yml` file,
switch the active program, export one back to disk, or delete one you no longer
need. Imported programs are copied into the app's own data store, so editing the
original file has no effect until you delete and re-import it.

The [`examples/`](examples) folder has a few more to try or copy from:

- [`20-20-20.yaml`](examples/20-20-20.yaml) — the bundled eye-care routine; repeats forever.
- [`pomodoro.yaml`](examples/pomodoro.yaml) — four 25-minute focus blocks with short breaks, then a long break.
- [`workout.yaml`](examples/workout.yaml) — a warm-up, two sprint/recover circuits at different paces, and a cool-down.
- [`advanced.yaml`](examples/advanced.yaml) — a circuit that reuses one round definition twice via a YAML anchor.

See [DSL.md](DSL.md) for the full language specification — durations, colors,
repeated blocks, and the program-wide `repeat` toggle.

## Development

Install the prerequisites from the [Tauri setup guide](https://v2.tauri.app/start/prerequisites/), including Node.js 22+, pnpm, Rust, and your platform's native build tools.

```bash
pnpm install
pnpm tauri dev
```

Useful commands:

```bash
pnpm check       # Validate versions and TypeScript
pnpm build       # Build the web frontend
pnpm tauri build # Build platform installers
```

## Releases

Update the version in `package.json`, `src-tauri/Cargo.toml`, and `src-tauri/tauri.conf.json`, then push a matching tag:

```bash
git tag v0.2.0
git push origin v0.2.0
```

GitHub Actions validates the project, builds Windows NSIS and MSI installers, and attaches them to a GitHub Release. Public Windows releases should be code-signed to avoid SmartScreen warnings.
