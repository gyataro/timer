# Timer

A minimal programmable interval timer for the desktop. Timer programs are small,
readable YAML files, and the included 20-20-20 eye-care routine works out of the
box. The timer continues running in the system tray when its window is hidden.

Built with [Tauri 2](https://v2.tauri.app/), TypeScript, Vite, and [Fluent UI Web Components](https://github.com/microsoft/fluentui/tree/master/packages/web-components).

## Timer programs

Open the program list from the timer window to import a `.yaml` or `.yml` file,
switch the active program, or delete an imported program. Imported programs are
copied into the application's data store; edit the original YAML in your preferred
text editor, then delete and import it again to replace the stored copy.

```yaml
Workout:
  activities:
    - 10s Get ready: marigold
    - 3x:
        - 30s Work out: red
        - 15s Rest: green
    - 30s Work out: red
    - 45s Cool down: lightBlue
```

See [DSL.md](DSL.md) for the complete language specification. To create programs
from natural-language instructions, copy [LLM_PROMPT.md](LLM_PROMPT.md) into an
LLM chat.

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
