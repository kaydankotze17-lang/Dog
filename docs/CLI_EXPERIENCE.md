# DOG CLI Experience & Custom Terminal Output

DOG includes a built-in, zero-dependency terminal messaging, theming, progress, and package lifecycle output system (`src/term/mod.rs`).

## 1. Terminal Capabilities & Graceful Fallbacks

DOG inspects the host terminal before rendering ANSI escapes or Unicode glyphs:
- **TTY Detection**: Uses `std::io::IsTerminal` on stdout/stderr. Interactive TTYs support in-place `\r` progress bar updates and ANSI colors. Non-TTY pipes emit clean milestone lines.
- **`NO_COLOR`**: Setting `NO_COLOR=1` disables all ANSI color sequences while preserving structured text badges (`[+ OK]`, `[! WARN]`, `[x ERR]`).
- **`TERM=dumb`**: Disables ANSI colors and automatically falls back to pure ASCII icons and progress bars (`[############----------]`, `| / - \`, `+--+`).
- **`CLICOLOR_FORCE=1` / `FORCE_COLOR=1`**: Forces ANSI colors even when piped.
- **Unicode / ASCII Fallback**: Inspects `LC_ALL`, `LC_CTYPE`, `LANG`, `TERMUX_VERSION`, and `DOG_ASCII=1`. Works cleanly on Android Termux (ARMv7 & AArch64), Linux, macOS, and Windows.

## 2. Terminal Themes, Prefixes & Timestamps

Configure persistent defaults with `dog config`:

```bash
dog config show
dog config theme kali          # default | kali | dracula | solar | mono
dog config timestamps on       # on | off
dog config prefix mybot        # custom [prefix] badge
dog config debug on            # on | off
dog config install-style detailed
```

## 3. DOG-Native CLI Output API (in `.dg` scripts)

| Built-in | Signature | Purpose |
| --- | --- | --- |
| `cli_info` | `(msg: text) -> void` | Styled informational message |
| `cli_success` | `(msg: text) -> void` | Styled success confirmation |
| `cli_warn` | `(msg: text) -> void` | Styled warning message |
| `cli_error` | `(msg: text) -> void` | Styled error message |
| `cli_debug` | `(msg: text) -> void` | Styled debug trace message |
| `cli_progress` | `(label: text, current: number, total: number) -> void` | Real-percentage progress bar |
| `cli_spinner` | `(step: number, label: text) -> void` | Step-driven Unicode/ASCII spinner frame |
| `cli_theme` | `(name: text) -> void` | Switch active theme (`default`, `kali`, `dracula`, `solar`, `mono`) |
| `cli_prefix` | `(prefix: text) -> void` | Set custom message prefix for the session |
| `cli_timestamps` | `(enabled: bool) -> void` | Toggle `[HH:MM:SS]` UTC timestamps |
| `cli_supports_color` | `() -> bool` | Query whether ANSI color is active |
| `cli_is_tty` | `() -> bool` | Query whether stdout is an interactive TTY |

## 4. Customizing Package Install & Uninstall Messages (`dog.pack`)

Package authors can customize lifecycle output during `dog add` and `dog remove` inside `dog.pack`:

```toml
[package]
name = "my_pkg"
version = "0.2.0"
entry = "src/main.dg"

[messages]
install_banner = "Installing MyPkg for DOG"
pre_install = "Checking required project layout..."
post_install = "Run `dog run src/main.dg` to try MyPkg!"
uninstall_banner = "Uninstalling MyPkg"
pre_uninstall = "Removing MyPkg dependency hooks..."
post_uninstall = "MyPkg has been removed cleanly."
```

`dog add` executes real dependency resolution, real file/byte transfer tracking (local directories, local cache, or `http(s)://` URLs), real SHA-256 tree verification into `dog.lock`, and displays an honest completion summary. `dog remove <pkg> [--purge]` checks reverse dependents and optionally purges cached files while reporting exact bytes freed.
