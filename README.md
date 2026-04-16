# arche-bin

Custom TUI system binaries for [arche](https://github.com/Dhruvpatel-10/arche) — an Arch Linux dotfiles and system configuration framework.

Uses the **Ember** color scheme, [ratatui](https://github.com/ratatui/ratatui) for rendering, and is optimized for minimal binary size (strip + LTO + `opt-level z`).

## Tools

### arche-legion

TUI system control panel for **Lenovo Legion Pro 5 16ARX8**. Replaces Lenovo Vantage.

- **Battery**: conservation mode (cap ~80%), capacity, health, power draw, cycle count
- **Performance**: platform profiles (low-power → max-power), CPU governor/EPP
- **Hardware toggles**: fan mode, camera kill switch, USB charging, Fn lock
- **Auth**: sudo password prompt for privileged sysfs writes
- Desktop notifications on toggle changes

## Build

Requires Rust (stable) and [just](https://github.com/casey/just).

```bash
just build         # build all (release)
just test          # run all tests
just deploy        # build + copy to ~/arche/tools/bin/
```

Per-tool targets: `just build-legion`, `just deploy-legion`.

## Deployment

Built binaries are copied into `~/arche/tools/bin/`. The arche bootstrap process symlinks them to their final locations:

- `arche-legion` → `~/.local/bin/arche/arche-legion` (user-local, Super+Ctrl+G)

## Project Structure

```
arche-bin/
├── Cargo.toml          # workspace root
├── Justfile            # build + deploy targets
└── arche-legion/
    ├── Cargo.toml
    └── src/
        └── main.rs     # full app (state, UI, sysfs I/O)
```

## License

Personal use. Part of the arche ecosystem.
