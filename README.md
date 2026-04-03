# arche-bin

Custom TUI system binaries for [arche](https://github.com/Dhruvpatel-10/arche) — an Arch Linux dotfiles and system configuration framework.

Both tools use the **Ember** color scheme, [ratatui](https://github.com/ratatui/ratatui) for rendering, and are optimized for minimal binary size (strip + LTO + `opt-level z`).

## Tools

### arche-greeter

TUI display manager for [greetd](https://sr.ht/~kennylevinsen/greetd/). Replaces `tuigreet`.

- Speaks greetd IPC directly via `greetd_ipc` crate
- Auto-detects Wayland/X11 sessions from `.desktop` files
- Remembers last user and session across reboots
- Caps lock warning, real-time clock, F2 shutdown
- ~670KB stripped binary

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

Per-tool targets: `just build-greeter`, `just deploy-legion`, etc.

## Deployment

Built binaries are copied into `~/arche/tools/bin/`. The arche bootstrap process symlinks them to their final locations:

- `arche-greeter` → `/usr/local/bin/arche-greeter` (system-wide, for greetd)
- `arche-legion` → `~/.local/bin/arche/arche-legion` (user-local, Super+Ctrl+G)

## Project Structure

```
arche-bin/
├── Cargo.toml          # workspace root
├── Justfile            # build + deploy targets
├── arche-greeter/
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs     # event loop
│       ├── app.rs      # state + logic
│       ├── ui.rs       # ratatui rendering
│       ├── ipc.rs      # greetd protocol
│       ├── session.rs  # .desktop file detection
│       ├── theme.rs    # ember color constants
│       └── error.rs    # error types
└── arche-legion/
    ├── Cargo.toml
    └── src/
        └── main.rs     # full app (state, UI, sysfs I/O)
```

## License

Personal use. Part of the arche ecosystem.
