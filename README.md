# Timer

A minimal, programmable interval timer for the desktop.

This was inspired by [jotaen/timer](https://github.com/jotaen/timer). I rebuilt it as a desktop app because I had too many browser tabs open to keep track of the timer.

It follows Windows 11's native UI conventions and keeps running in the system tray when its window is closed.

Built with [Tauri 2](https://v2.tauri.app/), TypeScript, Vite, and [Fluent UI Web Components](https://github.com/microsoft/fluentui/tree/master/packages/web-components).

## Getting started

Timer programs are YAML files describing a sequence of activities. For example, here's a 20-20-20 eye-break timer:

```yaml
20-20-20:
  repeat: true
  activities:
    - 20m Work: blue
    - 20s Break: green
```

See [DSL.md](DSL.md) for the full language specification, and the [`examples/`](examples) folder for a few more programs to try or copy from. Import a YAML file through the app to load it. The app can manage multiple programs, so you can switch between them anytime.

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
