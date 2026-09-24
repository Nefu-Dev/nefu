# Nefu

> **A tool that packages web projects into standalone desktop executables**
> Rust main version | v1.0.0

Nefu is a lightweight Web-to-Desktop packaging tool: it packages your HTML/CSS/JS project (or a `.nc` declarative UI file) into a **runtime-free** standalone desktop executable. Resource files are encrypted with AES-256-GCM and embedded into the binary; double-click to run, with no need to install Node.js, Electron, or any other environment.

---

## Table of Contents

- [Features](#features)
- [Project Structure](#project-structure)
- [Requirements and Build](#requirements-and-build)
- [Quick Start](#quick-start)
- [CLI Reference](#cli-reference)
- [Configuration File main.nefu](#configuration-file-mainnefu)
- [.nc Component Language](#nc-component-language)
- [JS Bridge API](#js-bridge-api)
- [Packaging and Security](#packaging-and-security)
- [Development Workflow](#development-workflow)
- [Example Projects](#example-projects)
- [FAQ](#faq)
- [License](#license)

---

## Features

| Feature | Description |
|------|------|
| 🔒 Encrypted Packaging | AES-256-GCM encryption for all resources, SHA-256 integrity verification |
| 🎨 .nc Language | JSON-based declarative UI with 45+ built-in Bootstrap 5 components |
| ⚡ Hot Reload | Dev server + WebSocket, files refresh automatically on change |
| 🌐 Cross-Platform | Windows (exe) / macOS (app) / Linux (bin) |
| 📦 Lightweight | Built on the system WebView (wry + tao), single-file distribution |
| 🔧 Bridge API | `lj()` / `nefu.invoke()` for JS ↔ Rust two-way communication |
| 🖥️ Window Management | Window state persistence, frameless, always-on-top, fullscreen, size limits |
| 📄 Custom Protocol | Resources load from memory via the `nefu://` protocol, never written to disk |
| 📝 Dual Config Formats | Supports both flat fields and `[app]/[build]/[window]` sections |

---

## Project Structure

```
nefu/
├── Cargo.toml          # Rust dependencies and release configuration
├── build.rs            # Windows icon embedding build script
├── src/                # Rust main version source
│   ├── main.rs         # Entry: command dispatch + packaged app self-start
│   ├── cli.rs          # Command-line argument parsing (clap)
│   ├── config.rs       # main.nefu config parsing and validation
│   ├── pack.rs         # Collect files → ZIP → AES encrypt → append to binary
│   ├── webview.rs      # Window and WebView lifecycle management
│   ├── server.rs       # Dev server (HTTP + WebSocket + file watching)
│   ├── protocol.rs     # nefu:// custom protocol handling
│   ├── bridge.rs       # JS-Rust IPC message protocol definitions
│   ├── nc_parser.rs    # .nc language → HTML compiler
│   └── utils.rs        # General utility functions
├── go/                 # Go language version (independent implementation)
├── examples/           # Example projects
│   ├── basic/          #   Basic HTML example (index.html)
│   └── nc-demo/        #   .nc component demo (index.nc)
├── website/            # Nefu official website (single-file index.html)
├── docs/               # Documentation
│   └── manual.md       #   Complete user manual
└── need/main.md        # Requirements and prompt summary
```

---

## Requirements and Build

### Requirements

- **Rust 1.70+** (requires cargo)
- **Windows**: WebView2 Runtime (usually built into Win10/11)
- **macOS**: WKWebView (built into the system)
- **Linux**: WebKitGTK (system packages required)

### Build

```bash
# Debug build
cargo build

# Release build (recommended for distribution; LTO / strip enabled)
cargo build --release

# The artifact is at target/release/nefu.exe (or target/debug/nefu.exe)
```

After building, copy the `nefu` executable to any project directory and use it (tool-oriented design: nefu can live anywhere; when packaging it automatically collects the files in the current project directory).

### Verify Installation

```bash
nefu version
# Nefu v1.0.0
# Web-to-Desktop packaging tool
# ...
```

### Logging

Control the log level via the `NEFU_LOG` environment variable:

```bash
NEFU_LOG=debug nefu start    # trace / debug / info / warn / error
```

---

## Quick Start

### Getting Started in 3 Steps

```bash
# 1. Initialize a project (creates main.nefu, index.nc, resources/, .gitignore)
nefu init

# 2. Start the dev server (hot reload, default port 3000)
nefu start

# 3. Package into an executable (exe on Windows, app on macOS, bin on Linux)
nefu build exe
# Output: dist/myapp.exe
```

Double-click `dist/myapp.exe` to run it — no runtime installation needed.

### Typical Project Structure

```
my-project/
├── main.nefu          # Configuration file (required)
├── index.html         # Entry page (index.nc also works)
├── preload.js         # Optional: bridge script executed before the page loads
├── assets/            # Static assets
└── dist/              # Build output (auto-generated)
```

---

## CLI Reference

### `nefu init`

Creates a project template in the current directory: `main.nefu`, `index.nc` (sample page), `resources/`, `.gitignore`.

```bash
nefu init
```

> Note: `init` does not accept a project name argument; it initializes directly in the current directory.

### `nefu start`

Starts the dev server and opens a desktop window (with built-in hot reload).

```bash
nefu start [OPTIONS]
```

| Option | Default | Description |
|------|--------|------|
| `-p, --port <PORT>` | `3000` | HTTP server port |
| `--host <HOST>` | `127.0.0.1` | Bind address (`0.0.0.0` means all interfaces) |
| `--no-reload` | `false` | Disable hot reload |
| `--open` | `true` | Automatically open the browser window on start |
| `--watch-extensions <EXTS>` | — | Additional file extensions to watch (comma-separated) |

**Behavior:**
1. Search upward for the project directory (containing `main.nefu`, up to 5 levels)
2. Start the HTTP file server + WebSocket hot-reload server + file watcher
3. Open a desktop window to load the page; files refresh automatically on change

### `nefu build <TARGET>`

Packages the project into a standalone executable.

```bash
nefu build exe [OPTIONS]     # Windows
nefu build app [OPTIONS]     # macOS
nefu build bin [OPTIONS]     # Linux
```

| Option | Description |
|------|------|
| `-c, --config <PATH>` | Custom config file path (default `main.nefu`) |
| `-o, --output-dir <DIR>` | Output directory (default `./dist`) |
| `--installer` | Generate an installer (placeholder, not fully implemented) |
| `--no-compress` | Do not compress resources (faster build, larger size) |
| `--no-encrypt` | Do not encrypt resources (debug only; the artifact contains plaintext source) |
| `-v, --verbose` | Verbose output |

**Build flow:**
1. Load the `main.nefu` configuration
2. Precompile all `.nc` files in the project to `.html`
3. Collect all project files (filtered by the `exclude` rules)
4. ZIP compress → AES-256-GCM encrypt → SHA-256 verify
5. Append to the nefu host executable → output to `dist/`

### `nefu version`

Shows version and feature information.

### `nefu help`

Shows help information.

---

## Configuration File main.nefu

`main.nefu` uses the TOML format. **Two writing styles are supported**; choose either:

1. **Flat fields** (tool default, recommended) — all fields flat at the top level
2. **Sectioned** (manual/legacy compatibility) — `[app]` / `[build]` / `[dev]` / `[window]` tables

### Complete Configuration (Flat Style)

```toml
# Entry HTML file path (default "index.html")
entry = "index.html"

# Output executable name (without extension, default "myapp")
output = "myapp"

# Application description
description = "My App"

# Version number
version = "1.0.0"

# Author
author = "Your Name"

# Debug mode (shows developer tools in debug builds)
debug = false

# Preload script path (optional, runs before the page loads)
preload = "preload.js"

# Window size
window_width = 1024
window_height = 768
min_width = 800
min_height = 600
max_width = 1920
max_height = 1080

# Window behavior
fullscreen = false
resizable = true
decorations = true          # false = frameless
always_on_top = false
title = "My App"            # Window title, defaults to output

# Custom User-Agent
user_agent = "nefu/1.0"

# Application icon path
icon = "assets/icon.png"

# Transparent window
transparent = false

# System tray icon (optional)
# tray_icon = "assets/tray.png"

# Security policy
allowed_domains = []
headers = {}
csp = "default-src 'self'"
context_menu = true
drag_drop = false

# Custom protocol name (default "nefu")
protocol = "nefu"

# Files excluded from packaging (glob patterns)
exclude = [
  "dist/**",
  ".nefu/**",
  "node_modules/**",
  "target/**",
  "*.log",
]

# Injected environment variables
env = { MY_KEY = "value" }

# Auto-update check URL (reserved)
# update_url = "https://example.com/update"

# Single instance lock (prevents multiple instances)
single_instance = false

# Window state persistence (remembers last position and size)
persist_window_state = true
```

### Sectioned Style (Legacy Compatible)

```toml
[app]
name = "My App"             # Maps to output
version = "1.0.0"
width = 1280                # Maps to window_width
height = 800                # Maps to window_height
resizable = true
fullscreen = false
icon = "assets/icon.png"

[build]
entry = "index.html"
preload = "preload.js"
output = "dist/myapp.exe"   # Uses the file stem, maps to output

[window]
title = "Main Window"
min_width = 800
min_height = 600
frameless = true            # Maps to decorations = false
always_on_top = false

[dev]
port = 3900
open = true
watch = true
```

### Default Exclude Rules

When `exclude` is not configured, the following are **not** packaged:

- Build artifacts: `dist/**`, `target/**`, `build/**`, `deps/**`, `*.exe`, `*.pdb`, `*.o`, `*.obj`
- Cache/version control: `.nefu/**`, `.git/**`, `.cargo/**`, `.idea/**`, `.vscode/**`, `node_modules/**`
- Logs and system files: `*.log`, `.DS_Store`, `Thumbs.db`

---

## .nc Component Language

`.nc` (Nefu Coding) is a JSON-based declarative UI language. Each `.nc` file describes a component tree, which Nefu compiles into a complete HTML page (automatically including Bootstrap 5.3, Font Awesome 6.5, and the Inter font).

### Basic Structure

```json
{
  "component": "page",
  "title": "My Page",
  "theme": "light",
  "children": [
    {
      "component": "container",
      "children": [
        { "component": "heading", "level": 1, "text": "Hello Nefu" },
        { "component": "button", "text": "Submit", "color": "primary" }
      ]
    }
  ]
}
```

### Two Attribute Styles (Can Be Mixed)

| Style | Example | Description |
|------|------|------|
| Manual style (flat) | `{ "component": "button", "text": "Submit", "color": "primary" }` | Attributes sit alongside `component` |
| init template style (wrapped) | `{ "type": "button", "props": { "class": "btn btn-primary" }, "children": [...] }` | Attributes go inside a `props` sub-object; the type field is `type` |

When the same field appears in both `props` and the top level, the **top-level flat field wins**.

### Component List (45+)

| Category | Components |
|------|------|
| Layout | `page`, `container`, `row`, `col`, `grid`, `divider`, `separator`, `spacer` |
| Navigation | `navbar`, `nav`, `tabs`, `accordion`, `breadcrumb`, `pagination`, `dropdown`, `carousel` |
| Forms | `form`, `input`, `textarea`, `select`, `checkbox`, `radio`, `radio-group` |
| Data | `table`, `list`, `list-group`, `progress`, `badge` |
| Feedback | `alert`, `modal`, `toast`, `tooltip`, `popover`, `spinner` |
| Content | `text`, `paragraph`, `heading`, `image`/`img`, `icon`, `code`, `html`, `raw` |
| Structure | `header`, `footer`, `sidebar`, `card`, `button` |

> Unknown component types do not cause errors; they are rendered as generic `div` elements with a warning.

### Compile Timing

- **Development**: `.nc` files are compiled to HTML in real time during `nefu start`
- **Build**: `nefu build` precompiles all `.nc` files in the project to `.html` before packaging

See [docs/manual.md](./docs/manual.md) Section 7 for the complete component attribute reference.

---

## JS Bridge API

A bridge script is injected before the page loads, exposing the global `nefu` object and the `lj()` shortcut function for JS ↔ Rust two-way communication.

### `lj(data)`

Sends data to Rust (equivalent to `nefu.send`):

```javascript
lj({ action: 'save', data: {...} });
```

### `nefu.invoke(method, args)`

Calls a Rust-side method and returns the value (Promise):

```javascript
const result = await nefu.invoke('readFile', ['/path/to/file.txt']);
```

### `nefu.send(data)`

Sends a message to Rust (no return value).

### `nefu.on(event, callback)`

Listens for Rust-triggered events and returns an unsubscribe function:

```javascript
const off = nefu.on('data-updated', (data) => {
    console.log('Data updated:', data);
});
// off();  // Unsubscribe
```

### `nefu.emit(event, data)`

Triggers a frontend event from the Rust side (normally not called manually).

### Ready Event

After the page's `DOMContentLoaded`, the `nefu-ready` custom event is fired; you can safely use the bridge API at that point:

```javascript
window.addEventListener('nefu-ready', () => {
    console.log('Nefu Bridge is ready');
});
```

### IPC Message Format

JS and Rust exchange JSON messages via `window.__nefu_ipc()`; see [bridge.rs](./src/bridge.rs) for the types:

| type | Direction | Description |
|------|------|------|
| `invoke` | JS → Rust | Call a method (contains `id`/`method`/`args`) |
| `send` | JS → Rust | Send data |
| `callback` | Rust → JS | Return a call result (matches Promises by `id`) |
| `event` | Rust → JS | Trigger a frontend event |

---

## Packaging and Security

### Binary Format

```
┌───────────────────────────────────────────────┐
│               nefu host executable             │
├───────────────────────────────────────────────┤
│   AES-256-GCM encrypted ZIP data               │
│   (contains [32B key][12B nonce][ciphertext],  │
│    or a plaintext ZIP)                         │
├───────────────────────────────────────────────┤
│   8 bytes   encrypted data length (u64 LE)     │
├───────────────────────────────────────────────┤
│   32 bytes  SHA-256 checksum                   │
├───────────────────────────────────────────────┤
│   8 bytes   magic "NEFUPACK"                   │
└───────────────────────────────────────────────┘
```

### Encryption Flow

1. Collect project files → ZIP compress (Deflated)
2. Generate a random 32-byte key + 12-byte nonce
3. AES-256-GCM encrypt, assembled as `[key][nonce][ciphertext]`
4. Compute the payload SHA-256 checksum
5. Concatenate `[host exe][payload][length][checksum][NEFUPACK]`

### Decryption/Verification Flow (when running a packaged artifact)

1. Read the file tail and verify the `NEFUPACK` magic
2. Read the length field and extract the encrypted data
3. Compute and compare SHA-256 (tamper protection)
4. Split key/nonce → AES-256-GCM decrypt → ZIP extract into **memory**
5. Serve resources to the WebView via the `nefu://` protocol, **never writing to disk**

### Notes

- `--no-encrypt` artifacts are plaintext ZIPs (starting with `PK\x03\x04`); at runtime they are auto-detected and decryption is skipped — **debug only, always encrypt for distribution**
- All resources stay in memory, and together with the magic check this effectively prevents simple tampering
- Every packaged file can be independently verified via `ResourcePack::unpack` / `verify_package`

---

## Development Workflow

### Hot Reload

`nefu start` starts three components:

- **HTTP file server** (tiny_http) — serves project static files
- **WebSocket hot-reload server** — notifies the frontend to refresh on change
- **File watcher** (notify) — watches extensions such as `html/htm/css/js/json/nc`

After saving a modified file, the page refreshes automatically — no manual restart needed.

### Debugging Tips

- `NEFU_LOG=debug nefu start` to see detailed logs
- `nefu build --no-encrypt --no-compress` to speed up debug builds
- With `debug = true` (in debug builds) the WebView developer tools are enabled automatically
- Window position/size is persisted automatically to the user data directory: `<APPDATA>/.nefu/<output>/window_state.json` (`~/.local/share/.nefu/...` on Linux)

### Common Build Issues

- **Port in use**: `nefu start -p 8080` to switch ports
- **Missing WebKitGTK on Linux**: install system packages such as `libwebkit2gtk-4.1-dev`
- **Double-clicking the packaged app does nothing**: first run `nefu start` to confirm the project loads, then debug with `cargo build` (debug build)

---

## Example Projects

| Directory | Description |
|------|------|
| `examples/basic/` | Traditional HTML project: `index.html` + `preload.js` + sectioned `main.nefu` |
| `examples/nc-demo/` | `.nc` component demo: `index.nc` showcasing 15+ components |

```bash
# Run the nc component demo
cd examples/nc-demo
nefu start          # Dev preview
nefu build exe      # Package
```

---

## FAQ

**Q1: What's the difference from Electron?**
Nefu uses the system's native WebView (wry/tao), so artifacts are usually only a few MB; Electron bundles Chromium + Node.js (about 150MB). The trade-off is that Nefu is more streamlined — no Node.js APIs on the JS side; system capabilities must be accessed through the bridge.

**Q2: Can packaged artifacts be distributed directly to users?**
Yes. Single file, no runtime dependencies, double-click to run (Windows requires the WebView2 Runtime to be installed, which is usually built into Win10/11).

**Q3: Does `.nc` support logic/conditional rendering?**
`.nc` is statically declarative. For dynamic logic, embed JS with the `html` component, or use plain HTML files.

**Q4: How large is the package?**
It depends on the project's resources. Simple HTML projects are usually 5–15MB (mostly the WebView runtime).

**Q5: Does encryption affect performance?**
Decryption happens at startup (usually <100ms); at runtime resources are read directly from memory, so there's no performance impact.

---

## License

MIT License

> See [docs/manual.md](./docs/manual.md) for the complete user manual (.nc component reference, error codes, FAQ, etc.).
