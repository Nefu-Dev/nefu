# Nefu User Manual

> **Nefu** — turn web pages into desktop apps  
> Version 1.3.0 (Rust implementation) | Last updated: 2026-09

---

## Table of Contents

1. [Project Introduction](#1-project-introduction)
2. [Feature Overview](#2-feature-overview)
3. [Installation and Build](#3-installation-and-build)
4. [Quick Start (3 minutes to get started)](#4-quick-start)
5. [CLI Reference](#5-cli-reference)
   - [nefu build .apk config overrides](#apk-target-options)
6. [main.nefu Configuration Reference](#6-mainnefu-configuration-reference)
7. [.nc Language Full Specification](#7-nc-language-full-specification)
   - [v2 New Syntax](#72-v2-new-syntax)
   - [Legacy Syntax (JSON Compatible)](#73-legacy-syntax-json-compatible)
8. [preload.js and the JS Bridge API](#8-preloadjs-and-the-js-bridge-api)
   - [SQL Management System](#sql-management-system)
   - [Fetching Web Page Source](#fetching-web-page-source)
9. [nefu:// Custom Protocol](#9-nefu-custom-protocol)
10. [Dev Server and Hot Reload](#10-dev-server-and-hot-reload)
11. [Packaging and Security](#11-packaging-and-security)
12. [Window Management and Rendering Optimization](#12-window-management-and-rendering-optimization)
13. [Development Workflow and Debugging](#13-development-workflow-and-debugging)
14. [Errors and Troubleshooting](#14-errors-and-troubleshooting)
15. [FAQ](#15-faq)
16. [Best Practices](#16-best-practices)

---

## 1. Project Introduction

Nefu is a lightweight tool that packages standard Web projects (HTML/CSS/JS) into standalone desktop executables. It does not require Node.js, Electron, or any other runtime dependency — the generated executable contains an embedded WebView engine (based on `wry` + `tao`) and runs by double-clicking.

### Core Philosophy

- **Simple**: one `main.nefu` config file + your web page = a desktop app
- **Secure**: AES-256-GCM encryption protects the source code, with SHA-256 integrity verification
- **Efficient**: hot reload in development, single-file distribution after packaging
- **Flexible**: the original `.nc` component language describes UI with JSON and ships 45+ built-in Bootstrap 5 components

### Use Cases

- Internal tools and admin panels
- Data dashboards and reporting systems
- Offline documentation and knowledge bases
- Lightweight desktop utilities
- Prototype demos and customer deliveries

---

## 2. Feature Overview

| Feature | Description |
|------|------|
| 🔒 Encrypted Packaging | AES-256-GCM encrypts all resources, SHA-256 integrity verification, `NEFUPACK` magic marker |
| 🎨 .nc Language | JSON-based declarative UI with 45+ built-in Bootstrap 5 components and two attribute styles |
| ⚡ Hot Reload | Dev server (HTTP + WebSocket), files refresh automatically on change, CSS updates without reload |
| 🌐 Cross-Platform | Windows (`exe`) / macOS (`app`) / Linux (`bin`) / Android (`.apk`) |
| 📦 Lightweight | Based on the system's native WebView, no external runtime dependency |
| 🔧 Bridge API | `lj()` / `nefu.invoke()` / `nefu.on()` two-way communication, 40+ built-in system methods (clipboard/dialog/file/window/storage/Shell/notification/system info/menu/update/screenshot, etc.) |
| 🖥️ Window Management | Window state persistence, frameless, always-on-top, fullscreen, size limits, transparent windows, system tray |
| 🛡️ Security Policy | CSP, domain whitelist, custom response headers, context menu control, drag-and-drop control, context isolation, secure IPC |
| 📦 Desktop Integration | System tray, single instance lock, macOS `.app` packaging, NSIS installer generation |
| 📄 Custom Protocol | Resources load from memory via the `nefu://` protocol, never written to disk |
| 📝 Dual Config Formats | Supports both flat fields and `[app]/[build]/[window]` sections |
| 🔍 Smart Development | Real-time `.nc` compilation, error diagnostic pages, automatic port avoidance, directory traversal protection |
| 🚀 Rendering Optimization | Auto-injected performance optimization script, high-performance / power-saving / auto rendering modes |
| ⚙️ Incremental Builds | File-hash-based build cache; unchanged files reuse previous artifacts |
| 📱 Android Support | `nefu build .apk` generates an Android Studio project to build the `.apk` installer |
| 🧵 Multithreading | Multi-threaded parallel file operations speed up builds |
| 💻 C/C++ Compilation | `nefu cpp` compiles `.c`/`.cpp` source files, auto-detecting the system compiler |
| 🖥️ **Electron-Level APIs** | Full Electron-compatible API namespaces, including `nefu.shell.*`, `nefu.notification.*`, `nefu.system.*`, `nefu.powerMonitor.*`, `nefu.updater.*`, `nefu.crashReporter.*`, `nefu.contextBridge.*`, `nefu.menu.*`, `nefu.captureScreenshot()` and more |
| 🧩 **Native Menus** | Full application menu bar (File/Edit/View/Window/Help) with shortcuts, submenus, separators, and roles |
| 🔔 **Desktop Notifications** | Cross-platform system-level desktop notifications with title/body/icon/timeout/urgency |
| 💻 **System Info** | CPU/memory/GPU/screen info/power state/system theme (dark/light modes) |
| 📂 **Shell Operations** | Open URL/path, reveal in file manager, move to trash, system beep, system version |
| 🔄 **Auto Update** | Check/download/install updates with progress callbacks |
| ⌨️ **Global Shortcuts** | Register/unregister global shortcuts with Electron-style shortcut syntax |
| 💥 **Crash Reporting** | Global panic capture, crash logs written to file, remote reporting |
| 🔐 **Context Bridge** | contextBridge-style secure isolation, whitelisted API exposure, message source verification |
| ⚙️ **Script Commands** | Define custom scripts in `main.nefu`, executed via `nefu run <name>` |
| 📦 **Automatic Dependency Management** | Auto-detects and downloads missing runtime dependencies (WebView2, NSIS, Android SDK, Java JDK) |
| 🖥️ **app Module** | Electron-like app module: app quit, restart, path lookup, name setting, etc. |
| 🔋 **Power Management** | `powerMonitor` idle detection + `powerSaveBlocker` to prevent system sleep |
| 🖼️ **Native Images** | `nativeImage` create/resize/crop images, multiple format conversions |
| ⚙️ **System Preferences** | `systemPreferences` for system colors, accessibility settings, languages, etc. |
| 🔐 **Secure Storage** | `safeStorage` encrypted storage for sensitive data with AES-256-GCM |
| 🌐 **Network Requests** | `net` module HTTP requests with GET/POST methods and custom headers |
| 📊 **Process Info** | `process` module for process memory usage, CPU usage, etc. |
| 📄 **webContents** | Access the page DOM, execute JS, manage navigation, set content, etc. |
| 📁 **session** | Manage HTTP sessions, cache, cookies, etc. |
| 🔗 **protocol** | Custom protocol registration and handling |
| 🖥️ **dock** | macOS Dock icon control (bounce, badge, etc.) |
| 🌗 **nativeTheme** | Listen for system theme changes, set theme mode |
| 📋 **contentTracing** | Performance tracing, records Chrome trace data |
| 🗄️ **SQL Management System** | Direct access and control of SQLite databases: query/execute/transaction/table inspection |
| 🌐 **Fetching Web Page Source** | Fetch page source, auto-detect encoding, extract title/meta/link/image stats |

---

## 3. Installation and Build

### 3.1 Requirements

- **Rust 1.70+** (with cargo)
- **Windows**: WebView2 Runtime (usually built into Win10/11)
- **macOS**: system-provided WKWebView
- **Linux**: install the WebKitGTK system packages (e.g. `libwebkit2gtk-4.1-dev`)

### 3.2 Build Steps

```bash
# Run in the repository root (contains Cargo.toml)
cargo build                 # Debug build → target/debug/nefu.exe
cargo build --release       # Release build → target/release/nefu.exe (LTO / strip enabled)
```

### 3.3 Tool-Oriented Usage

The compiled `nefu` executable is self-contained: **copy it into any project directory and use it**. When you run `build`, it automatically collects all files in the current project directory (itself excluded).

### 3.4 Automatic Dependency Management

Nefu auto-detects and downloads missing runtime dependencies when needed, with no manual installation:

| Dependency | Purpose | Auto-detection timing |
|------|------|-------------|
| **WebView2 Runtime** | Required to run Windows desktop apps | Checked when opening a desktop window; downloaded if missing |
| **NSIS (makensis)** | Generate Windows installers (`--installer`) | Checked at build time; downloaded from SourceForge if missing |
| **Android SDK** | Android builds (`build .apk`) | Checked at build time; prompts to download automatically |
| **Java JDK 17+** | Required for Android builds | Checked at build time; prompts to download |

Auto-downloaded dependencies are saved to the user's temp directory or a specified path; no manual environment variable configuration is needed.

### 3.5 Verify Installation

```bash
nefu version
# Nefu v1.0.0
# Web-to-Desktop packaging tool
# ...
```

### 3.6 Logging

Control the log level via the `NEFU_LOG` environment variable:

```bash
NEFU_LOG=debug nefu start     # trace / debug / info / warn / error
```

---

## 4. Quick Start

### 4.1 Getting Started in 3 Steps

```bash
# Step 1: initialize a project
nefu init

# Step 2: start the dev server (hot reload, default port 3000)
nefu start

# Step 3: package into an executable
nefu build exe               # Windows; use app on macOS and bin on Linux
```

Double-click `dist/myapp.exe` to run it — no runtime installation needed.

### 4.2 Interactive `nefu init`

`nefu init` uses an interactive prompt to guide you through project initialization:

```bash
nefu init
# Initialize a Nefu project in the current directory? (Y/n): Y
# Choose a template type:
#   1) basic - basic HTML/CSS/JS project
#   2) nc - uses the .nc declarative UI language (recommended)
# Enter template number (1/2): 2
# ✓ Project initialized!
```

If `main.nefu` already exists in the current directory, an overwrite warning is shown and confirmation is required.

The following files are created in the current directory:

| File | Description |
|------|------|
| `main.nefu` | Project configuration file (TOML format, fully commented) |
| `index.nc` | Sample page (.nc declarative UI) |
| `resources/` | Resource file directory |
| `.gitignore` | Git ignore rules |

### 4.3 Typical Project Structure

```
my-project/
├── main.nefu          # Configuration file (required)
├── index.html         # Entry page (index.nc also works)
├── preload.js         # Optional: bridge script executed before the page loads
├── assets/            # Static assets
├── resources/         # Resource directory (icons, etc.)
└── dist/              # Build output (auto-generated)
```

---

## 5. CLI Reference

### 5.1 `nefu init`

Initializes a new project interactively in the current directory, guiding you through template selection.

```bash
nefu init
```

### 5.2 `nefu run <NAME>`

Executes a script command defined in the `main.nefu` configuration file.

```bash
nefu run <NAME> [args...]
```

| Argument | Description |
|------|------|
| `NAME` | The script name to run (defined under `[scripts]` in `main.nefu`) |
| `args` | Extra arguments passed to the script |

**Examples:**

```bash
# Run the build script
nefu run build

# Run the deploy script with arguments
nefu run deploy --production --tag v1.0
```

Scripts run in the project directory and their output streams to the terminal in real time. If a script exits with a non-zero code, an error message is shown.

### 5.3 `nefu start`

Starts the dev server and opens a desktop window (with built-in hot reload).

```bash
nefu start [OPTIONS]
```

| Option | Default | Description |
|------|--------|------|
| `-p, --port <PORT>` | `3000` | HTTP server port |
| `--host <HOST>` | `127.0.0.1` | Bind address (`0.0.0.0` means all interfaces) |
| `--no-reload` | `false` | Disable hot reload |
| `--open` | `true` | Automatically open the desktop window on start |
| `--watch-extensions <EXTS>` | — | Additional file extensions to watch (comma-separated) |

**Behavior flow:**
1. Search upward from the current directory for the project (containing `main.nefu`, up to 5 levels)
2. Check the entry file; if the entry `.html` does not exist but a `.nc` file with the same name does, prompt and convert automatically
3. Start the HTTP file server + WebSocket hot-reload server + file watcher
4. Open a desktop window to load the page

### 5.4 `nefu build <TARGET>`

Packages the project into a standalone executable or an APK.

```bash
nefu build exe [OPTIONS]      # Windows (.exe)
nefu build app [OPTIONS]      # macOS (.app)
nefu build bin [OPTIONS]      # Linux (.bin)
nefu build .apk [OPTIONS]     # Android (.apk)
```

#### exe / app / bin Target Options

| Option | Description |
|------|------|
| `-c, --config <PATH>` | Custom config file path (default `main.nefu`) |
| `-o, --output-dir <DIR>` | Output directory (default `./dist`) |
| `--installer` | Generate an installer (the Windows `exe` target produces an NSIS script `<output>_installer.nsi`, compiled with `makensis`) |
| `--no-compress` | Do not compress resources (ZIP Store mode; faster build, larger size) |
| `--no-encrypt` | Do not encrypt resources (debug only; the artifact contains plaintext source) |
| `-v, --verbose` | Verbose output |

**Build flow (exe/app/bin):**
1. Parse and validate the build arguments (the target platform must be `exe`/`app`/`bin`/`.apk`)
2. Load the `main.nefu` configuration
3. Precompile all `.nc` files in the project to `.html`
4. Collect all project files (filtered by the `exclude` rules; nefu itself and `target/` are excluded automatically)
5. ZIP compress → AES-256-GCM encrypt → SHA-256 verify
6. Append to the nefu host executable → output `dist/<output>.<target>`

> Note: using `--no-compress --no-encrypt` together produces an artifact with unprotected source code; debug only.

#### .apk Target Options

| Option | Description |
|------|------|
| `-c, --config <PATH>` | Custom config file path (default `main.nefu`) |
| `-n, --name <NAME>` | App name (default `NefuApp`) |
| `-o, --output-dir <DIR>` | Output directory (default `./android`) |
| `-s, --set <KEY=VALUE>` | Config override (repeatable; dynamically modifies config parameters) |
| `--apk` | Fully automated APK build (auto-downloads JDK/Android SDK/Gradle) |
| `--release` | Build a release version (default is debug) |

**Config overrides (the `-s` option):**

You can temporarily override config parameters with `-s key=value` without modifying the `main.nefu` file. It can be used multiple times:

```bash
nefu build .apk -s entry=admin.html -s debug=true
nefu build .apk -s window_width=1280 -s window_height=800
```

Supported config keys: `entry`, `output`, `description`, `version`, `author`, `debug`, `window_width`, `window_height`, `fullscreen`, `resizable`, `decorations`, `always_on_top`, `single_instance`, `persist_window_state`, `context_menu`, `drag_drop`, `render_optimization`, `rendering_mode` (auto/high-performance/power-saving), `icon`, `title`, `csp`, `protocol`, `update_url`.

**Build flow (.apk):**
1. Compile `.nc` files to HTML
2. Copy all web resources into the Android project's `assets/web/` directory
3. Generate `AndroidManifest.xml`, `MainActivity.java`, `NefuBridge.java`, and other required files
4. Generate the `build.gradle` build script
5. Generate launcher icons, theme styles, string resources, etc.

**Next steps:**
1. Open the generated project directory with Android Studio
2. Wait for the Gradle sync to complete
3. Click Build → Build APK(s) to build the installer
4. The generated `.apk` file is at `app/build/outputs/apk/release/`

### 5.5 `nefu cpp`

Compiles C/C++ source files into executables.

```bash
nefu cpp <SOURCE> [OPTIONS] [-- <EXTRA_ARGS>...]
```

| Argument/Option | Description |
|-----------|------|
| `SOURCE` | C/C++ source file path (supports `.c` and `.cpp` files) |
| `-o, --output <PATH>` | Output file path (default: same name as the source, without extension) |
| `-- <EXTRA_ARGS>` | Extra compile arguments, e.g. `-O2`, `-std=c++17` |

**Examples:**

```bash
# Compile a C source file
nefu cpp main.c

# Compile a C++ source file with an output path and optimization options
nefu cpp main.cpp -o myapp -- -O2 -std=c++17

# Compile a C++ source file, linking external libraries
nefu cpp app.cpp -o app.exe -- -O2 -lcurl -lssl
```

**Behavior flow:**
1. Verify the source file exists and has a `.c` or `.cpp` extension
2. Auto-detect the system compiler (`.c` files try `gcc`/`clang`/`cc`; `.cpp` files try `g++`/`clang++`/`c++`)
3. Run the compile command and output to the specified path
4. Return the executable file path

> Note: C/C++ compilation depends on the system-installed GCC/Clang compiler; Nefu does not bundle a compiler. Library files such as `.h`/`.hpp`/`.a`/`.lib` are excluded automatically when packaging desktop apps and never end up in the final artifact.

### 5.6 `nefu version`

Shows version and feature information.

### 5.7 `nefu help` / `--help` / `-h`

Shows help information.

### 5.8 Exit Codes

- `0`: success
- `1`: execution failed (error message written to the log)

---

## 6. main.nefu Configuration Reference

`main.nefu` uses the TOML format and is the project's core configuration. **Two writing styles are supported**, detected automatically:

1. **Flat fields** (recommended): all fields flat at the top level
2. **Sectioned** (manual/legacy compatibility): `[app]` / `[build]` / `[dev]` / `[window]` tables

### 6.1 Complete Flat Configuration

```toml
# Entry HTML file path (default "index.html")
entry = "index.html"

# Output executable name (no extension, default "myapp"; letters/digits/-/_ only)
output = "myapp"

# Application description (default "Nefu App")
description = "My App"

# Application version (semantic versioning x.y.z recommended)
version = "1.0.0"

# Author information (optional)
author = "Your Name"

# Debug mode: enables WebView developer tools in debug builds
debug = false

# Preload script path (optional, runs before the page loads)
preload = "preload.js"

# Window size (width 200-7680, height 150-4320)
window_width = 1024
window_height = 768
min_width = 800
min_height = 600
max_width = 1920
max_height = 1080

# Window behavior
fullscreen = false           # Start fullscreen
resizable = true             # Resizable
decorations = true           # Show title bar/border (false = frameless)
always_on_top = false        # Always on top
title = "My App"             # Window title (defaults to output)
transparent = false          # Transparent window

# Custom User-Agent (default "nefu/1.0")
user_agent = "nefu/1.0"

# Application icon path (optional)
icon = "assets/icon.png"

# System tray icon (optional)
# tray_icon = "assets/tray.png"

# Security policy
allowed_domains = []                       # Allowed domain list
headers = {}                               # Extra HTTP response headers
csp = "default-src 'self'"                 # Content-Security-Policy
context_menu = true                        # Enable the context menu
drag_drop = false                          # Allow file drag-and-drop

# Custom protocol name (default "nefu", letters/digits/- only)
protocol = "nefu"

# Files/directories excluded at build time (glob patterns, see 6.4)
exclude = [
  "dist/**",
  ".nefu/**",
  "node_modules/**",
  "target/**",
  "*.log",
]

# Environment variables injected into the process
env = { MY_KEY = "value" }

# Auto-update check URL (silent background check, see 11.7)
# update_url = "https://example.com/update"

# Single instance lock (prevents multiple instances; exits with a notice if the lock fails)
single_instance = false

# Window state persistence (remembers last position/size/maximized state, default true)
persist_window_state = true

# Rendering optimization config
# rendering_mode: "auto" (default) | "high-performance" | "power-saving"
rendering_mode = "auto"
# Whether to auto-inject the rendering optimization script (default true)
render_optimization = true

# Custom script commands (run via nefu run <name>)
[scripts]
build = "npm run build"
deploy = "deploy.bat"
test = "cargo test"
```

### 6.2 Sectioned Style (Legacy Compatible)

```toml
[app]
name = "My App"               # → output
version = "1.0.0"
width = 1280                  # → window_width
height = 800                  # → window_height
resizable = true
fullscreen = false
icon = "assets/icon.png"

[build]
entry = "index.html"
preload = "preload.js"
output = "dist/myapp.exe"     # uses the file stem → output = "myapp"
compress = true               # informational only; requires --no-compress
encrypt = true                # informational only; requires --no-encrypt

[window]
title = "Main Window"
min_width = 800
min_height = 600
frameless = true              # → decorations = false
always_on_top = false

[dev]
port = 3900
open = true
watch = true

[scripts]
build = "npm run build"
deploy = "deploy.bat"
```

### 6.3 Configuration Validation Rules

The configuration is validated automatically when loaded; invalid values cause an error:

| Field | Rule |
|------|------|
| `entry` | Must not be empty |
| `output` | Letters, digits, hyphens, and underscores only |
| `window_width` | Must be between 200 and 7680 |
| `window_height` | Must be between 150 and 4320 |
| `min_width/max_width` | min must not be greater than max |
| `min_height/max_height` | min must not be greater than max |
| `protocol` | Non-empty; letters, digits, and hyphens only |
| `version` | A warning (not blocking) is issued if it does not match `x.y.z` |

### 6.4 Default Exclude Rules

When `exclude` is not configured, the following are **not** packaged:

- **Build artifacts**: `dist/**`, `target/**`, `build/**`, `deps/**`, `.fingerprint/**`, `.cargo/**`, `*.pdb`, `*.o`, `*.obj`, `*.exe`
- **Cache/version control**: `.nefu/**`, `.git/**`, `.idea/**`, `.vscode/**`, `node_modules/**`, `__pycache__/**`
- **Logs/system**: `*.log`, `.DS_Store`, `Thumbs.db`

> Glob wildcards are supported: `*` (no separators), `**` (across directories), `?` (single character).

### 6.5 Config File Search

`nefu start` / `nefu build` look for the configuration in the following priority order:
1. `main.nefu` in the current directory
2. `nefu.toml` in the current directory
3. Search parent directories (up to 5 levels)

---

## 7. .nc Language Full Specification

### 7.1 Overview

`.nc` (Nefu Coding) is a declarative UI scripting language supporting **two syntax styles**:

1. **v2 new syntax** (recommended): script-like syntax with comments, `import` statements, a `main"` main function block, and `nefu()` function calls
2. **Legacy syntax** (JSON compatible): a JSON-based declarative UI component tree

Nefu auto-detects the syntax type and compiles it into a complete HTML page, automatically including:
- **Bootstrap 5.3.0** (CSS + JS Bundle)
- **Font Awesome 6.5.0** icon library
- **Inter font** (Google Fonts)

### 7.2 v2 New Syntax

#### Basic Structure

```text
import<nefu.nch>        \ Import the main module
\ This is a single-line comment
/* This is a multi-line comment */
main"                    \ Main function
  nefu("list",[1,2,3,4],BLACK)  \ Call the nefu function
  nefu("alert","hello world")
end(all);                \ End
```

#### Syntax Rules

| Syntax | Description |
|------|------|
| `import<module>` | Import statement (must appear at the top of the file) |
| `\ comment` | Single-line comment; from `\` to end of line |
| `/* comment */` | Multi-line comment |
| `main"` or `main'` | Start of the main function block |
| `nefu("type", args...)` | Call a nefu function; auto-converts to `await nefu.invoke(...)` |
| `end(all);` | End of the main function block |

#### nefu() Function Calls

`nefu()` function calls are compiled into JavaScript `await nefu.invoke()` calls.

**Supported arguments:**
- **Strings**: `"hello"` or `'hello'`
- **Arrays**: `[1, 2, 3]`, `["a", "b"]`
- **Numbers**: `123`, `3.14`
- **Booleans**: `true`, `false`
- **Identifiers (constants)**: `BLACK`, `RED`, `ALL` (all-uppercase identifiers become quoted string constants automatically)

**Examples:**

```text
nefu("list",[1,2,3,4],BLACK)       → await nefu.invoke("list", [1, 2, 3, 4], "BLACK")
nefu("alert","hello")               → await nefu.invoke("alert", "hello")
nefu("dialog", "Confirm delete?", true)   → await nefu.invoke("dialog", "Confirm delete?", true)
```

### 7.3 Legacy Syntax (JSON Compatible)

The legacy syntax is a JSON-based declarative UI component tree; each `.nc` file describes one component tree.

#### Two Attribute Styles (Can Be Mixed)

| Style | Example | Description |
|------|------|------|
| **Manual style (flat)** | `{ "component": "button", "text": "Submit", "color": "primary" }` | Attributes sit alongside `component` |
| **init template style (wrapped)** | `{ "type": "button", "props": { "class": "btn btn-primary" }, "children": [...] }` | The type field is `type`; attributes go inside a `props` sub-object |

When the same field appears in both `props` and the top level, the **top-level flat field wins**. The component type field can be either `component` or `type`.

### 7.4 Common Attributes

All components support the following common attributes (flat or wrapped in `props`):

| Attribute | Type | Description |
|------|------|------|
| `component` / `type` | string | Component type (required) |
| `children` | array | Child component list (some components also accept `items`/`options`/`tabs`/`slides`/`footer`) |
| `class` | string | CSS class name |
| `id` | string | HTML id attribute |
| `style` | string | Inline styles |
| `onclick` / `onClick` | string | Click event expression (either case) |
| `onchange` / `onChange` | string | Value change event |
| `onsubmit` / `onSubmit` | string | Form submit event |

> Text content can go in `children` (as strings inside the array) or in the `text`/`content`/`label` attributes.

### 7.5 Component List

#### page / body — Page Root Component

Renders its children as the `<body>` HTML. Page metadata (`title`/`lang`/`css`/`js`/`head`) is read from the root node:

| Attribute | Type | Default | Description |
|------|------|--------|------|
| `title` | string | `Nefu App` | Page title |
| `lang` | string | `zh-CN` | Page language |
| `css` | string | — | Extra CSS link |
| `js` | string | — | Extra JS link |
| `head` | string | — | Extra raw HTML injected into `<head>` |

```json
{
  "component": "page",
  "title": "My App",
  "theme": "light",
  "children": [...]
}
```

#### button — Button

| Attribute | Type | Default | Description |
|------|------|--------|------|
| `text` / `label` | string | — | Button text (falls back to children) |
| `color` | string | `primary` | Color (primary/secondary/success/danger/warning/info/light/dark) |
| `outline` | bool | false | Outline style (btn-outline-*) |
| `size` | string | — | `sm` / `lg` |
| `loading` | bool | false | Loading state (disables the button) |
| `disabled` | bool | false | Disabled |
| `type` | string | `button` | Button type (button/submit/reset) |
| `dismiss` | bool | false | Close the modal on click (data-bs-dismiss) |
| `onClick` | string | — | Click event |

```json
{
  "component": "button",
  "text": "Submit",
  "color": "primary",
  "size": "lg",
  "onClick": "lj('submitForm')"
}
```

#### input — Input

| Attribute | Type | Default | Description |
|------|------|--------|------|
| `label` | string | — | Label text |
| `type` | string | `text` | text/email/password/number/tel/url/search/date/time, etc. |
| `placeholder` | string | — | Placeholder |
| `value` | string | — | Initial value |
| `name` | string | — | name attribute |
| `required` | bool | false | Required |
| `readonly` | bool | false | Read-only |
| `onChange` / `onInput` | string | — | Event |

#### textarea — Textarea

| Attribute | Type | Default | Description |
|------|------|--------|------|
| `label` | string | — | Label text |
| `placeholder` | string | — | Placeholder |
| `rows` | int | 3 | Number of rows |
| `text` / `content` | string | — | Content (falls back to children) |

#### select — Dropdown Select

| Attribute | Type | Description |
|------|------|------|
| `label` | string | Label text |
| `options` | array | Option list (children also accepted) |
| `multiple` | bool | Multiple selection |
| `onChange` | string | Selection event |

Option object: `{ "value": "val", "label": "Display text", "selected": true, "disabled": false }` (plain string options are also supported)

#### checkbox — Checkbox

| Attribute | Type | Description |
|------|------|------|
| `label` | string | Label text (falls back to children) |
| `checked` | bool | Whether checked |
| `name` / `required` | — | Form attributes |
| `onChange` | string | Event |

#### radio / radio-group — Radio Group

| Attribute | Type | Description |
|------|------|------|
| `name` / `id` | string | Group name |
| `label` | string | Group label (renders a fieldset/legend) |
| `options` | array | Option list |
| `inline` | bool | Horizontal layout |

#### form — Form

| Attribute | Type | Default | Description |
|------|------|--------|------|
| `action` | string | `#` | Submit URL |
| `method` | string | `POST` | Submit method |
| `onSubmit` | string | — | Submit event |

#### table — Table

| Attribute | Type | Default | Description |
|------|------|--------|------|
| `headers` | array | — | Column header names |
| `rows` | array | — | Data rows |
| `striped` | bool | false | Zebra striping |
| `hover` | bool | false | Hover highlight |
| `bordered` | bool | false | Borders |
| `responsive` | bool | false | Responsive scrolling |

Row object: `{ "cells": ["value1", "value2"], "actions": [{ "label": "Edit", "color": "primary", "onClick": "..." }] }`

#### card — Card

| Attribute | Type | Description |
|------|------|------|
| `title` | string | Card title (rendered as card-header) |
| `class` / `id` / `style` | — | Common attributes |

#### navbar — Navbar

| Attribute | Type | Default | Description |
|------|------|--------|------|
| `brand` | string | — | Brand text |
| `theme` | string | `light` | light/dark |
| `bg` | string | — | Background color (primary/secondary/...) |
| `sticky` | bool | false | Sticky top |
| `items` | array | — | Nav items |

Nav item object: `{ "label": "Text", "href": "#link", "active": true }`

#### container — Container

| Attribute | Type | Description |
|------|------|------|
| `fluid` | bool | Full-width container (container-fluid) |

#### row / col — Grid Layout

- `row`: `class` (default `row`)
- `col`: `size` (col-*), `sm` / `md` / `lg` / `xl` (responsive column widths, e.g. `col-md-6`), `class`

```json
{
  "component": "row",
  "children": [
    { "component": "col", "size": 6, "children": [...] },
    { "component": "col", "md": 6, "lg": 4, "children": [...] }
  ]
}
```

#### grid — Grid

| Attribute | Type | Default | Description |
|------|------|--------|------|
| `cols` | int | 3 | Columns per row (row-cols-*) |
| `gap` | int | 3 | Gap (g-*) |

#### text / paragraph — Text Paragraph

| Attribute | Type | Default | Description |
|------|------|--------|------|
| `tag` | string | `p` | HTML tag |
| `content` / `text` | string | — | Text content (falls back to children) |
| `onclick` | string | — | Event |

#### heading — Heading

| Attribute | Type | Default | Description |
|------|------|--------|------|
| `level` | int | 3 | Level (1-6, clamped automatically) |
| `text` | string | — | Heading text |

#### icon — Icon (Bootstrap Icons)

| Attribute | Type | Default | Description |
|------|------|--------|------|
| `name` | string | `bi-circle` | Icon class name (e.g. bi-heart) |
| `size` | int | 1 | Font size (rem) |
| `color` | string | — | Color |

#### image / img — Image

| Attribute | Type | Default | Description |
|------|------|--------|------|
| `src` | string | — | Image path |
| `alt` | string | — | Alt text |
| `rounded` | bool | false | Rounded corners |
| `circle` | bool | false | Circular |
| `thumbnail` | bool | false | Thumbnail border |
| `width` / `height` | string | — | Dimensions |

#### badge — Badge

| Attribute | Type | Default | Description |
|------|------|--------|------|
| `text` | string | — | Text (falls back to children) |
| `color` / `variant` | string | `primary` | Color |
| `pill` | bool | false | Pill shape |

#### progress — Progress Bar

| Attribute | Type | Default | Description |
|------|------|--------|------|
| `value` | number | 0 | Current value |
| `max` | number | 100 | Maximum value |
| `color` / `variant` | string | `primary` | Color |
| `striped` | bool | false | Striped |
| `animated` | bool | false | Animated |
| `label` | string | — | Display text (defaults to the percentage) |

#### spinner — Spinner

| Attribute | Type | Default | Description |
|------|------|--------|------|
| `type` | string | `border` | border / grow |
| `size` | string | — | `sm` |
| `color` / `variant` | string | `primary` | Color |
| `text` | string | `Loading...` | Accessibility text |

#### alert — Alert

| Attribute | Type | Default | Description |
|------|------|--------|------|
| `type` / `variant` | string | `primary` | info/success/warning/danger/primary, etc. |
| `dismissible` | bool | false | Dismissible |
| `text` | string | — | Content (falls back to children) |

#### modal — Modal

| Attribute | Type | Description |
|------|------|------|
| `id` | string | Modal ID (default `modal`) |
| `title` | string | Title |
| `size` | string | `sm` / `lg` / `xl` |
| `static` | bool | Static backdrop (does not close when clicking outside) |
| `footer` | array | Footer button list |

Footer button object: `{ "text": "OK", "color": "primary", "onClick": "...", "dismiss": true }`

#### tabs — Tabs

| Attribute | Type | Description |
|------|------|------|
| `id` | string | Tab group ID (default `tabs`) |
| `pills` | bool | Pill style |
| `tabs` | array | Tab list (children also accepted) |

Tab object: `{ "id": "tab1", "label": "Label", "active": true, "children": [...] }` (the first tab is active by default)

#### accordion — Accordion

| Attribute | Type | Description |
|------|------|------|
| `id` | string | Accordion ID (default `accordion`) |
| `flush` | bool | Borderless style |
| `items` | array | Collapsible item list (children also accepted) |

Collapsible item object: `{ "id": "item1", "title": "Title", "expanded": true, "children": [...] }`

#### carousel — Carousel

| Attribute | Type | Description |
|------|------|------|
| `id` | string | Carousel ID (default `carousel`) |
| `controls` | bool | Left/right arrows |
| `indicators` | bool | Indicators |
| `dark` | bool | Dark style |
| `fade` | bool | Fade transition |
| `slides` | array | Slides (children also accepted) |

Slide elements: an image path string, `{ "image": "...", "caption": "..." }`, or any component object.

#### dropdown — Dropdown Menu

| Attribute | Type | Description |
|------|------|------|
| `label` | string | Trigger button text (default `Dropdown`) |
| `color` / `variant` | string | Button color |
| `id` | string | Menu ID |
| `children` | array | Menu items (strings, or `{ "label"/"text", "href", "active" }`; `"---"` or `"divider"` renders a separator) |

#### list — Unordered List

| Attribute | Type | Description |
|------|------|------|
| `items` | array | List items (strings, or `{ "text"/"label" }`) |
| `class` | string | Custom class |

#### list-group — List Group

| Attribute | Type | Description |
|------|------|------|
| `items` | array | Array of list item objects |
| `flush` | bool | Borderless |
| `numbered` | bool | Numbered |
| `horizontal` | bool | Horizontal |

List item object: `{ "text": "Content", "badge": "Badge text", "active": true, "disabled": false, "variant": "danger" }`

#### pagination — Pagination

| Attribute | Type | Description |
|------|------|------|
| `size` | string | `sm` / `lg` |
| `align` | string | Alignment (start/center/end) |
| `children` | array | Page objects: `{ "text"/"label", "href", "active", "disabled" }` |

#### breadcrumb — Breadcrumb

| Attribute | Type | Description |
|------|------|------|
| `items` | array | Strings or `{ "text"/"label", "href" }`; the last item is highlighted automatically |

#### tooltip — Tooltip

| Attribute | Type | Default | Description |
|------|------|--------|------|
| `text` | string | — | Tooltip text (falls back to children) |
| `placement` | string | `top` | top/bottom/left/right |

#### popover — Popover

| Attribute | Type | Default | Description |
|------|------|--------|------|
| `title` | string | — | Title |
| `content` | string | — | Content |
| `placement` | string | `right` | Placement |
| `trigger` | string | `click` | Trigger method |

#### toast — Toast

| Attribute | Type | Default | Description |
|------|------|--------|------|
| `title` | string | `Notification` | Title |
| `subtitle` | string | — | Subtitle |
| `autohide` | bool | false | Auto-hide |
| `delay` | int | `5000` | Display duration (ms) |
| `children` | array | — | Body content |

#### offcanvas — Offcanvas Drawer

| Attribute | Type | Default | Description |
|------|------|--------|------|
| `title` | string | — | Title |
| `placement` | string | `start` | start/end/top/bottom |
| `backdrop` | bool | true | Close on backdrop click |

#### nav — Navigation

| Attribute | Type | Description |
|------|------|------|
| `pills` / `fill` / `justified` | bool | Style options |

#### header / footer — Page Header / Footer

| Attribute | Type | Description |
|------|------|------|
| `text` | string | Content (falls back to children) |
| `class` / `id` / `style` | — | Common attributes |

#### sidebar — Sidebar

| Attribute | Type | Default | Description |
|------|------|--------|------|
| `width` | string | `250px` | Width |
| `position` | string | `left` | left / right (maps to float) |

#### divider / separator — Divider

Renders `<hr>`. Supports `class` / `style`.

#### spacer — Spacer

| Attribute | Type | Default | Description |
|------|------|--------|------|
| `width` | string | `10px` | Width |
| `height` | string | `10px` | Height |

#### html — Raw HTML

| Attribute | Type | Description |
|------|------|------|
| `content` | string | Raw HTML (not escaped) |

#### raw — Raw Content

| Attribute | Type | Description |
|------|------|------|
| `content` / `text` | string | Raw content (not escaped, no wrapping tags) |

#### code — Code Block

| Attribute | Type | Description |
|------|------|------|
| `language` | string | Programming language (generates a language-* class) |
| `content` / `text` | string | Code content |

#### Generic/Unknown Components

Unknown component types do not cause errors; they are rendered as generic `div` elements with a warning. Safe tags such as `span`/`section`/`main`/`article`/`ul`/`ol`/`li`/`h1-h6` can also be used directly as `component` names.

### 7.5 Page Metadata and HTML Skeleton

A non-`page` root component is wrapped into a complete HTML page automatically (including Bootstrap/Font Awesome/Inter and the tooltip/popover initialization script). The root node supports:

| Attribute | Description |
|------|------|
| `title` | Page title |
| `lang` | Page language |
| `css` | Extra CSS link |
| `js` | Extra JS script link |
| `head` | Extra raw HTML injected into the head |

### 7.6 Validation Rules

- The root node must be a JSON object
- Must contain a `component` or `type` field
- Nesting depth must not exceed 50 levels
- `props` must be an object
- If a single child component fails to render, that node is skipped and rendering continues without affecting the whole page

---

## 8. preload.js and the JS Bridge API

### 8.1 Overview

A bridge script is injected before the page loads, exposing the global `nefu` object and the `lj()` shortcut function for JS ↔ Rust two-way communication. When `preload = "preload.js"` is configured, your preload code runs after the bridge script.

### 8.2 Global Objects

| Object | Description |
|------|------|
| `window.nefu` | Main bridge API object |
| `window.lj(data)` | Shortcut function for sending data (equivalent to `nefu.send`) |
| `window.__nefu_ipc(json)` | Low-level IPC channel (the real implementation is injected by wry) |

### 8.3 nefu Object Methods

| Method | Description |
|------|------|
| `nefu.invoke(method, args)` | Call a Rust-side method and return a Promise |
| `nefu.send(data)` | Send data to Rust (no return value) |
| `nefu.on(event, callback)` | Listen for Rust-triggered events; returns an unsubscribe function |
| `nefu.emit(event, data)` | Trigger an event (called by the Rust side) |
| `nefu.getInfo()` | Get version, platform, and User-Agent information |

### 8.3.1 Electron-Style API Namespaces

Nefu provides complete Electron-compatible API namespaces usable directly in JavaScript:

```javascript
// === Shell system operations ===
nefu.shell.openExternal('https://example.com');        // Open a URL
nefu.shell.openPath('/path/to/folder');                // Open a path
nefu.shell.showItemInFolder('/path/to/file.txt');      // Reveal the file location
nefu.shell.beep();                                     // System beep
nefu.shell.trashItem('/path/to/file.txt');             // Move to trash
nefu.shell.getSystemVersion();                         // Get the system version

// === Desktop notifications ===
nefu.notification.send({ title: 'Notification', body: 'Content' });
nefu.notification.isSupported();

// === System info ===
nefu.system.getTheme();                                // System theme (dark/light)
nefu.system.getMemoryInfo();                           // Memory info
nefu.system.getCpuInfo();                              // CPU info
nefu.system.getGpuInfo();                              // GPU info
nefu.system.getPowerState();                           // Power state
nefu.system.getScreenInfo();                           // Screen info
nefu.system.getSummary();                              // System summary
nefu.system.refresh();                                 // Refresh the cache

// === Power monitoring ===
nefu.powerMonitor.getSystemIdleState(5000);            // Idle state

// === Auto update ===
nefu.updater.checkForUpdates('https://example.com/update.json');

// === Crash reporting ===
nefu.crashReporter.generateTestReport();

// === Context bridge ===
nefu.contextBridge.isAvailable();

// === Native menus ===
nefu.menu.getDefaultTemplate('MyApp');                 // Get the menu template

// === Screenshot ===
const base64 = await nefu.captureScreenshot();         // Fullscreen screenshot

// === Printing ===
nefu.print();                                          // Print
nefu.printToPDF();                                     // Export to PDF

// === app module ===
nefu.app.quit();                                       // Quit the app
nefu.app.relaunch();                                   // Relaunch the app
nefu.app.getVersion();                                 // Get the version number
nefu.app.getPath('userData');                          // Get the user data directory
nefu.app.isPackaged();                                 // Whether the app is packaged
nefu.app.getLocale();                                  // Get the system locale

// === Power management ===
nefu.powerMonitor.getSystemIdleTime();                 // Idle time
nefu.powerSaveBlocker.start('prevent-display-sleep');  // Prevent display sleep

// === Screen info ===
nefu.screen.getAllDisplays();                          // All displays
nefu.screen.getCursorScreenPoint();                    // Mouse position

// === Native images ===
nefu.nativeImage.createFromPath('/path/to/image.png');
nefu.nativeImage.resize({ width: 200, height: 200 });

// === System preferences ===
nefu.systemPreferences.isDarkMode();                   // Dark mode
nefu.systemPreferences.getAccentColor();               // Accent color

// === Secure storage ===
nefu.safeStorage.encryptString('sensitive data');
nefu.safeStorage.decryptString('encrypted_base64');

// === Network requests ===
const resp = await nefu.net.fetch('https://api.example.com');
const data = await nefu.net.get('https://api.example.com/data');

// === Process info ===
nefu.process.getMemoryInfo();                          // Process memory
nefu.process.getCPUUsage();                            // CPU usage

// === Page content control ===
nefu.webContents.loadURL('https://example.com');
nefu.webContents.executeJavaScript('document.title');
nefu.webContents.reload();

// === Session management ===
nefu.session.clearCache();
nefu.session.getCookies().then(cookies => console.log(cookies));

// === Custom protocols ===
nefu.protocol.registerStringProtocol('myapp', (req) => {
  return { data: 'Hello from custom protocol!' };
});

// === macOS Dock ===
nefu.dock.setBadge('3');                               // Set the badge
nefu.dock.bounce('critical');                          // Bounce the icon

// === Theme ===
nefu.nativeTheme.shouldUseDarkColors();                // Whether dark
nefu.nativeTheme.themeSource('dark');                  // Set dark theme

// === Performance tracing ===
nefu.contentTracing.startRecording({ categoryFilter: '*' });
// ... perform actions ...
nefu.contentTracing.stopRecording().then(data => console.log(data));
```

These API namespaces are registered automatically when the bridge initializes; no extra configuration is needed.

```javascript
// Call a method
const result = await nefu.invoke('fs.readFile', ['./data/config.json']);

// Listen for an event
const off = nefu.on('data-updated', (data) => {
    console.log('Data updated:', data);
});
// off(); // Unsubscribe

// Send data
lj({ action: 'save', data: {...} });
```

### 8.4 Ready Event Notification

After the page's `DOMContentLoaded`, the `nefu-ready` custom event is fired:

```javascript
window.addEventListener('nefu-ready', () => {
    console.log('Nefu Bridge is ready');
    nefu.invoke('getVersion').then(v => console.log('Version:', v));
});
```

### 8.5 Built-in Rust Methods

Called via `nefu.invoke('<method>', [args])`. All built-in methods can be extended or overridden with `Bridge::register_method`:

#### Basic Methods

| Method | Args | Description |
|------|------|------|
| `getVersion` | — | Returns the Nefu version number |
| `getPlatform` | — | Returns the platform (windows/macos/linux/unknown) |
| `getTime` | — | Returns the current timestamp (ms) |
| `echo` | data | Echoes data back (for testing) |
| `log` | message | Rust-side INFO log `[JS Log]` |
| `warn` | message | Rust-side WARN log `[JS Warn]` |
| `error` | message | Rust-side ERROR log `[JS Error]` |
| `openUrl` | url | Opens an external URL (http/https only, security-restricted) |
| `print` | — | Prints the current page |
| `printToPDF` | — | Prints to PDF |
| `captureScreenshot` | — | Captures a fullscreen screenshot, returns a base64-encoded PNG |

#### Clipboard

| Method | Args | Description |
|------|------|------|
| `clipboard.read` | — | Reads clipboard text |
| `clipboard.write` | text | Writes clipboard text |
| `clipboard.readImage` | — | Reads a clipboard image (returns base64, may be empty) |
| `clipboard.hasImage` | — | Checks whether the clipboard has an image |

#### Window Control

| Method | Args | Description |
|------|------|------|
| `window.minimize` | — | Minimizes the window |
| `window.maximize` | — | Maximizes the window |
| `window.close` | — | Closes the window |

#### File System (Restricted)

| Method | Args | Description |
|------|------|------|
| `fs.readFile` | path | Reads a file (restricted; see the security notes below) |
| `fs.writeFile` | path, content | Writes a file (restricted; see the security notes below) |
| `fs.exists` | path | Checks whether a file/directory exists |

#### Dialogs

| Method | Args | Description |
|------|------|------|
| `dialog.alert` | message | Shows an alert box |
| `dialog.confirm` | message | Shows a Yes/No confirmation box, returns a boolean |
| `dialog.openFile` | options | Opens a file picker, returns an array of paths (`options.multiple` controls multi-select) |
| `dialog.saveFile` | options | Opens a save dialog, returns a path (`options.defaultName` sets the default file name) |

#### Local Storage

| Method | Args | Description |
|------|------|------|
| `storage.get` | key | Reads local storage |
| `storage.set` | key, value | Writes local storage |
| `storage.remove` | key | Removes a local storage key |

#### Shell System Operations (like the Electron shell module)

| Method | Args | Description |
|------|------|------|
| `shell.openExternal` | url | Opens a URL in the default browser (http/https/mailto only) |
| `shell.openPath` | path | Opens a path in the file manager |
| `shell.showItemInFolder` | path | Reveals and selects a file in the folder |
| `shell.beep` | — | Plays the system beep |
| `shell.trashItem` | path | Moves a file to the trash |
| `shell.getSystemVersion` | — | Gets the OS version number |

#### Desktop Notifications (like the Electron Notification API)

| Method | Args | Description |
|------|------|------|
| `notification.send` | options | Sends a desktop notification; options contain title/body/icon/timeout/urgent fields |
| `notification.isSupported` | — | Checks whether the system supports desktop notifications |

#### System Info (like the Electron systemPreferences / screen modules)

| Method | Args | Description |
|------|------|------|
| `system.getTheme` | — | Gets the system theme (dark/light/unknown) |
| `system.getMemoryInfo` | — | Gets memory info (total/available/used_percent/swap) |
| `system.getCpuInfo` | — | Gets CPU info (name/cores/usage_percent/arch) |
| `system.getGpuInfo` | — | Gets GPU info (name/vram/driver/is_integrated) |
| `system.getPowerState` | — | Gets power state (on_battery/battery_percent/charging) |
| `system.getScreenInfo` | — | Gets primary screen info (width/height/scale_factor) |
| `system.getSummary` | — | Gets a system info summary (JSON object) |
| `system.refresh` | — | Refreshes the system info cache |

#### Power Monitoring (like the Electron powerMonitor module)

| Method | Args | Description |
|------|------|------|
| `powerMonitor.getSystemIdleState` | idleThreshold | Gets system idle state and power info |

#### Auto Update (like the Electron autoUpdater module)

| Method | Args | Description |
|------|------|------|
| `updater.checkForUpdates` | updateUrl | Checks for updates, returns the update status (checking/update_available/up_to_date/error) |

#### Crash Reporting (like the Electron crashReporter module)

| Method | Args | Description |
|------|------|------|
| `crashReporter.generateTestReport` | — | Generates a test crash report |

#### Context Bridge (like the Electron contextBridge module)

| Method | Args | Description |
|------|------|------|
| `contextBridge.isAvailable` | — | Checks whether the context bridge is available |

#### Native Menus (like the Electron Menu module)

| Method | Args | Description |
|------|------|------|
| `menu.getDefaultTemplate` | appName | Gets the default app menu template (JSON), including File/Edit/View/Window/Help menus |

#### app Module (like the Electron app module)

| Method | Args | Description |
|------|------|------|
| `app.quit` | — | Quits the app |
| `app.exit` | exitCode | Force-quits with the given exit code |
| `app.relaunch` | — | Relaunches the app |
| `app.getName` | — | Gets the app name |
| `app.getVersion` | — | Gets the app version number |
| `app.getPath` | name | Gets an app path (home/appData/userData/exe/temp/desktop/documents/downloads/music/pictures/videos) |
| `app.setName` | name | Sets the app name |
| `app.isPackaged` | — | Whether running in packaged mode |
| `app.getLocale` | — | Gets the system locale |
| `app.getLocaleCountryCode` | — | Gets the country code |
| `app.isDefaultProtocolClient` | protocol | Checks whether it is the default protocol handler |
| `app.setDefaultProtocolClient` | protocol | Sets it as the default protocol handler |

#### powerMonitor Module (like the Electron powerMonitor module)

| Method | Args | Description |
|------|------|------|
| `powerMonitor.getSystemIdleState` | idleThreshold | Gets system idle state (idle/active/unknown) and power info (on battery/battery percent/charging state) |
| `powerMonitor.getSystemIdleTime` | — | Gets system idle time (seconds) |

#### powerSaveBlocker Module (like the Electron powerSaveBlocker module)

| Method | Args | Description |
|------|------|------|
| `powerSaveBlocker.start` | type | Prevents the system from entering power-saving mode (prevent-display-sleep/prevent-app-suspension), returns the blocker ID |
| `powerSaveBlocker.stop` | id | Releases the specified power-save blocker |
| `powerSaveBlocker.isStarted` | id | Checks whether the specified power-save blocker is running |

#### screen Module (like the Electron screen module)

| Method | Args | Description |
|------|------|------|
| `screen.getCursorScreenPoint` | — | Gets the mouse cursor position (x/y) |
| `screen.getPrimaryDisplay` | — | Gets primary display info (size/scale/rotation/color depth, etc.) |
| `screen.getAllDisplays` | — | Gets a list of all displays |
| `screen.getDisplayNearestPoint` | x, y | Gets the display containing the given coordinates |
| `screen.getDisplayMatching` | rect | Gets the display with the largest intersection with the given rectangle |
| `screen.getScreenSize` | — | Gets the total screen size (all displays combined) |

#### nativeImage Module (like the Electron nativeImage module)

| Method | Args | Description |
|------|------|------|
| `nativeImage.createEmpty` | — | Creates an empty image |
| `nativeImage.createFromPath` | path | Creates an image from a file path |
| `nativeImage.createFromDataURL` | dataUrl | Creates an image from a Data URL |
| `nativeImage.createFromBuffer` | buffer, options | Creates an image from a Buffer |
| `nativeImage.resize` | options | Resizes an image (width/height/quality) |
| `nativeImage.crop` | rect | Crops an image (x/y/width/height) |
| `nativeImage.getSize` | — | Gets image dimensions (width/height) |
| `nativeImage.toDataURL` | — | Converts to a Data URL |
| `nativeImage.toPNG` | — | Converts to PNG format |
| `nativeImage.toJPEG` | quality | Converts to JPEG format |

#### systemPreferences Module (like the Electron systemPreferences module)

| Method | Args | Description |
|------|------|------|
| `systemPreferences.getSystemColor` | color | Gets a system color (e.g. highlight, accent, etc.) |
| `systemPreferences.isDarkMode` | — | Checks whether dark mode is enabled |
| `systemPreferences.isHighContrastColorScheme` | — | Checks whether a high-contrast color scheme is in use |
| `systemPreferences.getColor` | name | Gets the value of a system color |
| `systemPreferences.getAccentColor` | — | Gets the system accent color |
| `systemPreferences.getUserDefault` | key, type | Gets a user default setting |
| `systemPreferences.getLanguageCode` | — | Gets the system language code |
| `systemPreferences.getSystemLanguage` | — | Gets the system language name |
| `systemPreferences.isAeroGlassEnabled` | — | Checks whether Aero Glass is enabled (Windows) |
| `systemPreferences.getEffectiveAppearance` | — | Gets the effective appearance mode |
| `systemPreferences.isSwipeTrackingFromScrollEventsEnabled` | — | Checks whether scroll-trackpad gesture tracking is enabled |

#### safeStorage Module (like the Electron safeStorage module)

| Method | Args | Description |
|------|------|------|
| `safeStorage.isEncryptionAvailable` | — | Checks whether the system supports encrypted storage |
| `safeStorage.encryptString` | plaintext | Encrypts a string, returns base64-encoded ciphertext |
| `safeStorage.decryptString` | encrypted | Decrypts a string, accepts base64-encoded ciphertext, returns plaintext |

#### net Module (like the Electron net module)

| Method | Args | Description |
|------|------|------|
| `net.fetch` | url, options | Makes an HTTP request; options support method/headers/body; returns status code, headers, and body |
| `net.get` | url, options | HTTP GET request shortcut |
| `net.post` | url, options | HTTP POST request shortcut |
| `net.isOnline` | — | Checks whether the network is available |

#### process Module (like the Electron process module)

| Method | Args | Description |
|------|------|------|
| `process.getMemoryInfo` | — | Gets process memory info (RSS/heap/stack/private memory, etc.) |
| `process.getCPUUsage` | — | Gets process CPU usage |
| `process.getProcessId` | — | Gets the process PID |
| `process.getIOCounters` | — | Gets process IO statistics |

#### webContents Module (like the Electron webContents module)

| Method | Args | Description |
|------|------|------|
| `webContents.getTitle` | — | Gets the page title |
| `webContents.getURL` | — | Gets the current page URL |
| `webContents.loadURL` | url | Loads the given URL |
| `webContents.loadFile` | path | Loads a local file |
| `webContents.goBack` | — | Navigates back |
| `webContents.goForward` | — | Navigates forward |
| `webContents.reload` | — | Reloads the page |
| `webContents.stop` | — | Stops loading |
| `webContents.executeJavaScript` | code | Executes JavaScript code in the page |
| `webContents.setZoomFactor` | factor | Sets the zoom factor |
| `webContents.getZoomFactor` | — | Gets the current zoom factor |
| `webContents.setZoomLevel` | level | Sets the zoom level |
| `webContents.getZoomLevel` | — | Gets the current zoom level |
| `webContents.setUserAgent` | ua | Sets a custom User-Agent |
| `webContents.getUserAgent` | — | Gets the current User-Agent |
| `webContents.openDevTools` | — | Opens the developer tools |
| `webContents.closeDevTools` | — | Closes the developer tools |
| `webContents.isDevToolsOpened` | — | Checks whether the developer tools are open |
| `webContents.canGoBack` | — | Checks whether it can go back |
| `webContents.canGoForward` | — | Checks whether it can go forward |
| `webContents.isLoading` | — | Checks whether the page is loading |
| `webContents.isLoadingMainFrame` | — | Checks whether the main frame is loading |
| `webContents.isCrashed` | — | Checks whether the page has crashed |
| `webContents.clearHistory` | — | Clears navigation history |

#### session Module (like the Electron session module)

| Method | Args | Description |
|------|------|------|
| `session.getCacheSize` | — | Gets the cache size (bytes) |
| `session.clearCache` | — | Clears the cache |
| `session.clearStorageData` | options | Clears storage data (supports storages/quota options) |
| `session.flushStorageData` | — | Flushes storage data |
| `session.setProxy` | config | Sets proxy configuration |
| `session.resolveProxy` | url | Resolves proxy information |
| `session.getCookies` | — | Gets all cookies |
| `session.setCookie` | cookie | Sets a cookie (must contain url/name/value fields) |
| `session.deleteCookie` | url, name | Deletes the given cookie |
| `session.clearCookies` | — | Clears all cookies |
| `session.getUserAgent` | — | Gets the default User-Agent |
| `session.setUserAgent` | ua | Sets a custom User-Agent |
| `session.isPersistent` | — | Checks whether the session is persistent |

#### protocol Module (like the Electron protocol module)

| Method | Args | Description |
|------|------|------|
| `protocol.registerFileProtocol` | scheme, handler | Registers a file protocol |
| `protocol.registerBufferProtocol` | scheme, handler | Registers a Buffer protocol |
| `protocol.registerStringProtocol` | scheme, handler | Registers a string protocol |
| `protocol.registerHttpProtocol` | scheme, handler | Registers an HTTP proxy protocol |
| `protocol.unregisterProtocol` | scheme | Unregisters a protocol |
| `protocol.isProtocolHandled` | scheme | Checks whether a protocol is registered |
| `protocol.interceptFileProtocol` | scheme, handler | Intercepts a file protocol |
| `protocol.interceptStringProtocol` | scheme, handler | Intercepts a string protocol |
| `protocol.interceptBufferProtocol` | scheme, handler | Intercepts a Buffer protocol |
| `protocol.interceptHttpProtocol` | scheme, handler | Intercepts an HTTP protocol |
| `protocol.uninterceptProtocol` | scheme | Removes a protocol interception |

#### dock Module (like the Electron app.dock module, macOS only)

| Method | Args | Description |
|------|------|------|
| `dock.bounce` | type | Bounces the Dock icon (critical/informational), returns the bounce ID |
| `dock.cancelBounce` | id | Cancels the bounce |
| `dock.setBadge` | text | Sets the Dock badge text |
| `dock.getBadge` | — | Gets the Dock badge text |
| `dock.hide` | — | Hides the Dock icon |
| `dock.show` | — | Shows the Dock icon |
| `dock.isVisible` | — | Checks whether the Dock icon is visible |
| `dock.setMenu` | menuItems | Sets the Dock menu (JSON array with label/disabled fields) |

#### nativeTheme Module (like the Electron nativeTheme module)

| Method | Args | Description |
|------|------|------|
| `nativeTheme.shouldUseDarkColors` | — | Checks whether dark colors should be used |
| `nativeTheme.shouldUseHighContrastColors` | — | Checks whether high-contrast colors should be used |
| `nativeTheme.shouldUseInvertedColorScheme` | — | Checks whether an inverted color scheme should be used |
| `nativeTheme.themeSource` | source | Sets/gets the theme source (system/light/dark) |

#### contentTracing Module (like the Electron contentTracing module)

| Method | Args | Description |
|------|------|------|
| `contentTracing.getCategories` | — | Gets all available trace categories |
| `contentTracing.startRecording` | options | Starts recording traces (options support categoryFilter/traceOptions) |
| `contentTracing.stopRecording` | — | Stops recording and returns trace data (base64-encoded JSON) |
| `contentTracing.getTraceBufferUsage` | — | Gets trace buffer usage (value/maximum percentage) |
| `contentTracing.isRecording` | — | Checks whether recording is in progress |

#### SQL Management System

Access and control SQLite databases directly through the `nefu.sql.*` methods:

| Method | Args | Description |
|------|------|------|
| `sql.open` | path | Opens/creates a SQLite database file, returns connection success info |
| `sql.openMemory` | — | Creates an in-memory database (temporary; data disappears when the app closes) |
| `sql.close` | — | Closes the current database connection |
| `sql.query` | sql | Runs a SELECT query, returns a JSON array (each row is a `{column: value}` object) |
| `sql.execute` | sql | Runs INSERT/UPDATE/DELETE/CREATE commands, returns affected row count and last insert ID |
| `sql.executeBatch` | sql | Runs multiple SQL statements (separated by semicolons), returns success info |
| `sql.beginTransaction` | — | Begins a transaction |
| `sql.commit` | — | Commits the transaction |
| `sql.rollback` | — | Rolls back the transaction |
| `sql.getInfo` | — | Gets database info (SQLite version, table list, connection path, etc.) |
| `sql.getTableInfo` | tableName | Gets column info for the given table (PRAGMA table_info) |
| `sql.listConnections` | — | Lists all saved connection names and the current connection |

**Example:**

```javascript
// Open the database
await nefu.invoke('sql.open', ['mydb.sqlite']);

// Create a table
await nefu.invoke('sql.execute', ['CREATE TABLE IF NOT EXISTS users (id INTEGER PRIMARY KEY, name TEXT, age INTEGER)']);

// Insert data
await nefu.invoke('sql.execute', ["INSERT INTO users (name, age) VALUES ('Zhang San', 25)"]);

// Query data
const users = await nefu.invoke('sql.query', ['SELECT * FROM users']);
console.log(users); // [{ "id": 1, "name": "Zhang San", "age": 25 }]

// Get database info
const info = await nefu.invoke('sql.getInfo', []);
console.log('Version:', info.version, 'Tables:', info.tables);

// Transactions
await nefu.invoke('sql.beginTransaction', []);
await nefu.invoke('sql.execute', ["UPDATE users SET age = 26 WHERE id = 1"]);
await nefu.invoke('sql.commit', []);

// Close the connection
await nefu.invoke('sql.close', []);
```

#### Fetching Web Page Source

Fetch and parse web page source through the `nefu.web.*` methods:

| Method | Args | Description |
|------|------|------|
| `web.fetch` | url | Fetches and parses the page source; returns JSON with status code, headers, HTML, title, Meta, stats, etc. |
| `web.fetchRaw` | url | Fetches the raw page source (unparsed), returns JSON containing the HTML source |

**`web.fetch` return structure:**

```json
{
  "success": true,
  "url": "https://example.com",
  "status": 200,
  "status_text": "OK",
  "headers": {
    "content-type": "text/html",
    "content-length": "1256",
    "server": "nginx"
  },
  "encoding": "UTF-8",
  "html": "<!DOCTYPE html>...",
  "size_bytes": 1256,
  "title": "Example Domain",
  "meta": {
    "description": "Example website description",
    "keywords": "example, test"
  },
  "stats": {
    "links": 10,
    "images": 2,
    "scripts": 1,
    "styles": 2
  }
}
```

**Example:**

```javascript
// Fetch the web page source
const result = await nefu.invoke('web.fetch', ['https://www.example.com']);
if (result.success) {
  console.log('Page title:', result.title);
  console.log('Encoding:', result.encoding);
  console.log('Link count:', result.stats.links);
  console.log('HTML source:', result.html);
} else {
  console.error('Fetch failed:', result.error);
}
```

**Security restrictions**: the URL must use the `http://` or `https://` scheme; a missing scheme prefix is completed automatically; the request times out after 30 seconds with a maximum response body of 10MB.

**Local storage** (`storage.*`): each key is one JSON file in the user data directory (Windows: `%APPDATA%/<output>`, macOS: `~/Library/Application Support/<output>`, Linux: `~/.local/share/<output>`); key names are sanitized.

**Dialogs** (`dialog.*`): use native system dialogs (rfd), no extra dependencies.

**Security restrictions:**
- `fs.readFile` / `fs.writeFile`: paths containing `..` are rejected; only files under the `./`, `data/`, `resources/`, and `assets/` prefixes are allowed
- `openUrl` / `shell.openExternal`: only `http://` / `https://` / `mailto:` schemes are allowed

### 8.6 IPC Message Format

JS and Rust communicate with JSON messages; the `type` field distinguishes message types:

| type | Direction | Structure | Description |
|------|------|------|------|
| `invoke` | JS → Rust | `{ type, id, method, args }` | Call a method |
| `send` | JS → Rust | `{ type, data }` | Send data |
| `callback` | Rust → JS | `{ type, id, result?, error? }` | Return a call result (matches Promises by id) |
| `event` | Rust → JS | `{ type, event, data }` | Trigger a frontend event |

---

## 9. nefu:// Custom Protocol

In packaged mode, all resources load through the custom protocol from the **in-memory resource pack** — no filesystem access, nothing written to disk.

### 9.1 Protocol Format

```
nefu://<host>/<path/to/resource>[?query][#fragment]
```

Examples: `nefu://localhost/index.html`, `nefu://localhost/css/style.css`

- Host: `localhost` (ignored)
- An empty path or `/` maps to the entry file (default `index.html`)
- The protocol name can be customized with `protocol = "xxx"` in the config

### 9.2 Resource Responses

- MIME types are set automatically by extension (html/css/js/json/png/svg/font/wasm and 30+ more)
- `Access-Control-Allow-Origin: *` and `Cache-Control: no-cache` are appended by default
- Missing resources return a 404 page

### 9.3 Cache Policy (Programmable Extension)

`ProtocolHandler` supports `CachePolicy`: `NoCache` (default) / `ShortLived` / `Immutable` / `Custom`, plus MIME type overrides, for secondary development.

---

## 10. Dev Server and Hot Reload

### 10.1 Architecture

`nefu start` starts three components at once:

| Component | Technology | Description |
|------|------|------|
| HTTP file server | tiny_http | Serves project static files (binds `0.0.0.0:port`) |
| WebSocket hot-reload server | self-implemented RFC 6455 | Standalone port, pushes change notifications to all connections |
| File watcher | notify | Recursively watches project directory file changes |

### 10.2 Hot Reload Behavior

- Watched extensions: `html`, `htm`, `css`, `js`, `mjs`, `json`, `nc`, `svg`, `png`, `jpg`, `jpeg`, `gif`, `webp`, `wasm` (extendable with `--watch-extensions`)
- Change push format: `{ "type": "reload", "file": "path" }`
- **CSS changes**: stylesheets update without a page reload (with a timestamp to bust the cache)
- **Other changes**: full page reload
- Auto-reconnect on disconnect (exponential backoff, up to 10s)

### 10.3 Real-Time .nc Compilation

- When `.html` is requested, if a `.nc` file with the same name exists, the `.nc` is **compiled in real time first** (avoids stale build artifacts)
- Editing and saving a `.nc` file takes effect on the next refresh
- When `.nc` parsing fails, a diagnostic page with the specific error is returned (HTTP 400) and auto-refreshes every 3 seconds, recovering automatically once fixed

### 10.4 Smart Features

- **Path safety**: URL decoding + directory traversal (`..`) detection
- **CORS**: responses include `Access-Control-Allow-Origin: *`
- **Cache control**: development mode forces `no-cache`
- **Port avoidance**: `find_available_port` can find the next available port automatically
- **Domain whitelist**: with `allowed_domains` configured, requests with a non-whitelisted `Host` get a `403`
- **Security response headers**: when `csp` / `headers` are configured, they are appended to every response

---

## 11. Packaging and Security

### 11.1 Packaged File Format

```
┌───────────────────────────────────────────────┐
│               nefu host executable            │
├───────────────────────────────────────────────┤
│   AES-256-GCM encrypted ZIP data              │
│   (contains [32B key][12B nonce][ciphertext]) │
├───────────────────────────────────────────────┤
│   8 bytes   encrypted data length (u64 LE)    │
├───────────────────────────────────────────────┤
│   32 bytes  SHA-256 checksum                  │
├───────────────────────────────────────────────┤
│   8 bytes   magic "NEFUPACK"                  │
└───────────────────────────────────────────────┘
```

### 11.2 Packaging Flow (build)

1. Collect project files (filtered by `exclude`; nefu itself and `target/` excluded)
2. ZIP compress (Deflated; Store when `--no-compress` is used)
3. Generate a random 32-byte key + 12-byte nonce
4. AES-256-GCM encrypt the ZIP data, assembled as `[key][nonce][ciphertext]`
5. Compute the payload SHA-256 checksum
6. Read the nefu host executable and append the payload in the format above to the output file

### 11.2.1 Incremental Build Cache

Nefu supports a file-hash-based incremental build cache that greatly speeds up repeated builds:

**How it works:**
1. Before building, compute the aggregate hash of all input files (including file paths, content, and build options)
2. Read the cache metadata from `dist/.nefu_cache/cache_meta.json`
3. If the hash matches and the cached artifact exists, copy the cached file directly as the output, skipping time-consuming steps like compression and encryption
4. If the hash does not match, run a full build and update the cache

**Usage:**
- No extra configuration needed; enabled automatically
- The cache is also updated accordingly in `--no-compress` or `--no-encrypt` modes
- Delete the `dist/.nefu_cache/` directory to force a rebuild

### 11.3 Runtime Flow (Unpacking)

1. Read the file tail and verify the `NEFUPACK` magic
2. Read the length field and extract the encrypted data (validating the length)
3. Compute and compare SHA-256 (tamper protection; mismatches are an error)
4. Split key/nonce → AES-256-GCM decrypt
5. ZIP extract into the **in-memory** resource pack (`ResourcePack`, with LRU cache)
6. Serve resources to the WebView via the `nefu://` protocol

### 11.4 Plaintext Mode Compatibility

`--no-encrypt` artifacts are plaintext ZIPs (starting with `PK\x03\x04`). At runtime this is auto-detected and decryption is skipped — **debug only, always encrypt for distribution**.

### 11.5 Integrity Verification and Utility Functions

| Function | Description |
|------|------|
| `verify_package(data)` | Verifies only the magic, length, and checksum without decrypting |
| `extract_package_info(data)` | Extracts package metadata (original exe size, encrypted data size, checksum) |
| `is_nefu_package(path)` | Reads only the last 8 bytes to determine whether a file is a Nefu package |

### 11.6 Security Notes

- The encryption key is generated randomly for every build and stored with the payload (tamper-resistant, but not a defense against professional reverse engineering)
- Consider extra packing (e.g. an obfuscator) or code signing for sensitive apps
- All resources stay in memory; nothing is written to disk
- `fs.readFile` / `openUrl` both have path and scheme whitelist restrictions

### 11.7 macOS `.app` Packaging

`nefu build app` produces the standalone executable `dist/<output>.app` and also generates a standard macOS application bundle:

```
<output>.app/
└── Contents/
    ├── Info.plist          # App metadata (CFBundleIdentifier, etc.)
    └── MacOS/
        └── <output>        # Executable (with the resource payload appended)
```

### 11.8 NSIS Installer

`nefu build exe --installer` generates `<output>_installer.nsi` (an NSIS install script) in the output directory, containing installation, Start-menu shortcut, and uninstaller logic. Compile it with NSIS's `makensis` to get `Setup.exe`:

```bash
makensis dist/<output>_installer.nsi
```

> Note: `--installer` currently supports only the Windows `exe` target.

### 11.9 Auto-Update Check

With `update_url = "https://example.com/update"` configured, the app silently checks for updates in the background at startup (asynchronous, does not block startup). It requests the URL to get the remote version number (JSON `{"version":"x.y.z"}` or plain text), compares it with the local version, and logs when a new version is found. If `update_url` is not configured, the check is skipped.

### 11.10 Dev Server Security Policy

The `allowed_domains`, `csp`, and `headers` settings apply to both the dev server and packaged runtime:

- **Domain whitelist**: when `allowed_domains` is non-empty, requests whose `Host` is not whitelisted get `403 Forbidden`
- **CSP**: when `csp` is non-empty, a `Content-Security-Policy` header is added to every response
- **Custom response headers**: each key-value pair in `headers` is appended to the response headers

---

## 12. Window Management and Rendering Optimization

### 12.1 Window Options

See config `[6.1]`: size, min/max limits, fullscreen, resizable, frameless (`decorations`), always-on-top, title, transparent, app icon (`icon`), window drag-and-drop (`drag_drop`), context menu (`context_menu`).

### 12.2 Window State Persistence

With `persist_window_state = true` (default), the position, size, and maximized state are saved automatically when the window closes and restored on the next launch.

State file locations (distinguished by the `output` name):

| Platform | Path |
|------|------|
| Windows | `%APPDATA%\.nefu\<output>\window_state.json` |
| macOS | `~/Library/Application Support/.nefu/<output>/window_state.json` |
| Linux | `~/.local/share/.nefu/<output>/window_state.json` |

### 12.3 Window Lifecycle

- Close request: save state → exit the event loop
- Debug builds (`debug = true` with a debug compilation) enable the developer tools automatically

### 12.4 System Tray

With `tray_icon = "assets/tray.png"` configured, the system tray is enabled:

- Single click on the tray icon → show/focus the window
- Right-click the tray menu → `Show Window` / `Exit`
- When the window is closed, the program stays in the tray; use `Exit` in the tray menu to fully quit

### 12.5 Single Instance

With `single_instance = true`, a file lock prevents multiple instances. If the lock fails at startup, another instance is already running; the program shows a notice and exits.

### 12.6 Window Control (JS Bridge)

The frontend can control the window through the bridge API:

```javascript
await nefu.invoke('window.minimize');  // Minimize
await nefu.invoke('window.maximize');  // Toggle maximize
await nefu.invoke('window.close');     // Close the window
```

### 12.7 Rendering Optimization

Nefu provides powerful rendering optimization to help with performance issues caused by large numbers of DOM elements:

**Configuration options:**
```toml
# Rendering mode: "auto" | "high-performance" | "power-saving"
rendering_mode = "auto"

# Whether to enable rendering optimization (default true)
render_optimization = true
```

**Rendering modes:**
- `auto`: automatically chooses based on system performance (default)
- `high-performance`: 60fps frame-rate limit, keeps all animations
- `power-saving`: 30fps frame-rate limit, reduces animations to save power

**Auto-injected optimizations:**
1. GPU-accelerated compositing layers (`translateZ(0)`, `will-change`)
2. Smooth scrolling support
3. CSS animation optimization (fewer repaints and reflows)
4. Large-list detection (warns in the console above a threshold)
5. Visibility-based lazy loading (`IntersectionObserver`)
6. Respects the user's `prefers-reduced-motion` setting

**Developer API:**
```javascript
// Check the current rendering mode
console.log(window.__nefu_render_mode);  // "auto" | "high-performance" | "power-saving"

// Frame-rate limit
console.log(window.__nefu_fps_limit);  // 0 = unlimited

// Virtual scroll threshold
console.log(window.__nefu_virtual_scroll_threshold);
```

**Performance CSS classes:**
```html
<!-- GPU acceleration -->
<div class="nefu-gpu-accelerated">...</div>

<!-- Container with lots of content -->
<div class="nefu-large-list">...</div>

<!-- Virtualized container -->
<div class="nefu-virtual-scroll-container">...</div>
```

---

## 13. Development Workflow and Debugging

### 13.1 Recommended Flow

```bash
nefu init               # Initialize
# Edit index.nc to design the UI
nefu start              # Dev preview (hot reload)
nefu build exe          # Package for release
```

### 13.2 Debugging Tips

- `NEFU_LOG=debug nefu start` to see detailed request/unpack logs
- `nefu build --no-encrypt --no-compress` to speed up debug builds
- The dev server adds an `X-Nefu-Dev: true` header to every response for easy identification
- Use the browser/WebView developer tools to inspect the DOM and network requests
- `.nc` compile errors appear on a diagnostic page and reload automatically

### 13.3 Suggested Project Structure

```
my-project/
├── main.nefu          # Configuration file
├── index.html         # or index.nc
├── preload.js         # Bridge script (optional)
├── assets/            # Static assets
│   ├── css/
│   ├── js/
│   └── images/
├── resources/         # Resource directory
└── dist/              # Build output (auto-generated)
```

---

## 14. Errors and Troubleshooting

### 14.1 Common Errors

| Error message | Cause | Solution |
|----------|------|----------|
| `Config file not found: <path>` | main.nefu not found | Run `nefu init` or enter the project directory |
| `Failed to parse the config file` | TOML syntax error / field type error | Check the main.nefu format |
| `window_width must be between 200-7680` | Window size out of range | Fix the config |
| `output name can only contain letters, digits, hyphens, and underscores` | Invalid output name | Fix the config |
| `Nefu project directory not found` | No config in the current directory | Run it from the project directory |
| `Entry file '<x>' does not exist` | The file pointed to by entry is missing | Check the entry path |
| `NC file is not valid JSON` | .nc JSON syntax error | Validate with a JSON tool |
| `NC root node is missing 'component' or 'type'` | The node lacks a type field | Add the component type |
| `Component nesting depth exceeds 50 levels` | Components nested too deeply | Split the structure |
| `Invalid magic identifier` | Not a Nefu package / corrupted file | Rebuild |
| `SHA-256 checksum mismatch` | File tampered with or corrupted | Rebuild / re-download |
| `AES-256-GCM decryption failed` | Corrupted data | Rebuild |
| `Cannot bind to port <port>` | Port in use | `nefu start -p 8080` |
| `Unsupported build target '<x>'` | Target is not exe/app/bin | Use a correct target |
| `UI frozen or unresponsive (IPC queue overload)` | Too many JS calls fill the IPC queue and block the event loop | Auto-fixed: at most 50 messages are processed per frame; remaining messages continue on the next frame, so the event loop is no longer blocked |
| `IPC message parse failed` | The JS side sent a malformed message (e.g. wrong argument types) | Check the argument format of `nefu.invoke()` calls in JS; make sure args is an array |
| `SQL database operation failed` | Wrong database path, insufficient permissions, or SQL syntax error | Check the database path, table names, and SQL statement syntax |

### 14.2 Built-in Error Pages

| Scenario | Status code | Description |
|------|--------|------|
| Dev server 404 | 404 | File-not-found page |
| .nc compile error | 400 | Page with error diagnostics, auto-reloads every 3 seconds |
| Protocol resource 404 | 404 | Missing-resource page in packaged mode |

---

## 15. FAQ

**Q1: What's the difference between Nefu and Electron?**
Nefu uses the system's native WebView (wry/tao), so artifacts are usually only a few MB; Electron bundles Chromium + Node.js (about 150MB). The trade-off is that Nefu is more streamlined — no Node.js APIs on the JS side; system capabilities go through the bridge.

**Q2: Can packaged artifacts be distributed directly to users?**
Yes. Single file, no runtime dependencies, double-click to run (Windows requires the WebView2 Runtime to be installed).

**Q3: Which operating systems are supported?**
Windows (exe), macOS (app), Linux (bin). Each platform uses its corresponding system WebView.

**Q4: Can .nc and HTML be mixed?**
Yes. The same project can use `.html` and `.nc` files together; in development, requesting `.html` automatically compiles the same-named `.nc` first, and at build time all `.nc` files are precompiled to `.html`.

**Q5: How large is the packaged file?**
It depends on the project's resources. Simple HTML projects are usually 5–15MB (mostly the WebView runtime).

**Q6: Can I use custom CSS/JS in .nc?**
Yes. Add custom classes with the `class` attribute; set `css`/`js`/`head` on the root node; or embed `<style>`/`<script>` with the `html` component.

**Q7: How do I handle multiple pages?**
Do SPA routing in JS in the entry page, or use the `html`/`raw` components to embed links to other `.html`/`.nc` pages (`.nc` files are converted automatically at build time).

**Q8: What if the port is in use?**
Use `nefu start -p 8080` to pick another port.

**Q9: Does encryption affect performance?**
Decryption happens only at startup (usually <100ms); at runtime resources are read from memory directly, so there is no impact.

**Q10: How do I update a released app?**
Run `nefu build` again to produce a new executable and replace the old one.

**Q11: Does .nc support conditional rendering and loops?**
`.nc` is statically declarative. For dynamic logic, embed JS with the `html` component, or use plain HTML files.

**Q12: Can I access Node.js APIs in preload.js?**
No. System features are implemented through the `nefu.invoke()` built-in methods (files, clipboard, window, storage, etc.).

**Q13: How do I add an icon?**
Configure `icon = "assets/app.ico"` (.ico recommended on Windows). `tray_icon` is the tray icon.

**Q14: Can I exclude files when packaging?**
Yes. Build artifacts/version control/logs are excluded by default; add custom glob rules with the `exclude` array.

**Q15: How do I report a bug?**
Attach the `NEFU_LOG=debug` log output, the config file, and reproduction steps.

**Q16: Which Electron-style APIs does Nefu support?**
Nefu implements many Electron-compatible APIs, including: `app` (app management), `shell` (system operations), `notification` (desktop notifications), `system` (system info), `powerMonitor` (power monitoring), `powerSaveBlocker` (power-save blocker), `screen` (screen info), `nativeImage` (native images), `systemPreferences` (system preferences), `safeStorage` (secure storage), `net` (network requests), `process` (process info), `webContents` (page control), `session` (session management), `protocol` (custom protocols), `dock` (macOS Dock control), `nativeTheme` (theme management), `contentTracing` (performance tracing), `menu` (native menus), `updater` (auto updates), `crashReporter` (crash reporting), `contextBridge` (context bridge), `globalShortcut` (global shortcuts), etc. It also ships a built-in **SQL Management System** (`sql.*` to operate SQLite databases directly) and **web page source fetching** (`web.fetch` to crawl page content). See [Section 8.5](#85-built-in-rust-methods).

**Q17: How do I define and run scripts in main.nefu?**
Add a `[scripts]` section to `main.nefu`:
```toml
[scripts]
build = "npm run build"
deploy = "deploy.bat"
```
Then run them with `nefu run build` or `nefu run deploy`. Scripts run in the project directory, support passing arguments, and stream output in real time.

**Q18: Does Nefu auto-download missing dependencies?**
Yes. Nefu has built-in automatic dependency management that detects and downloads missing runtime components (WebView2, NSIS, Android SDK, Java JDK) when needed. No manual installation or environment variable configuration is required.

---

## 16. Best Practices

### 16.1 Project Organization

- Put static assets in the `assets/` subdirectory
- Avoid Chinese file names (compatibility issues with some toolchains)
- Split complex pages into multiple `.nc` / `.html` files

### 16.2 Performance Optimization

- Use WebP for images and compress them appropriately
- Compress CSS/JS for production
- Avoid loading too many resources in the entry file

### 16.3 Security Recommendations

- Always encrypt production builds (do not use `--no-encrypt`)
- Do not hardcode sensitive credentials in preload.js
- Protect user input against XSS
- Use the `csp` config to restrict resource origins

### 16.4 .nc Authoring Guidelines

- Keep JSON indented with 2 spaces
- Keep component nesting under 5 levels (hard limit is 50)
- Add meaningful `id`s to interactive elements
- Use camelCase consistently for event names (`onClick`/`onChange`)

### 16.5 Version Management

- Update `version` before releasing (semantic versioning recommended)
- Keep historical build artifacts for rollback
- Use `exclude` to omit `dist/` and keep source under version control

### 16.6 Script Automation

- Define repetitive tasks (build, deploy, test, clean) as `[scripts]` config entries
- Scripts support cross-platform execution (`.bat`/`.ps1` on Windows, `.sh` on Unix)
- Combine with CI/CD toolchains for automated builds and releases

### 16.7 Electron API Migration Guide

When migrating an Electron app to Nefu:
- Use `nefu.invoke('app.method', args)` instead of `electron.app.method()`
- Use `nefu.invoke('shell.method', args)` instead of `electron.shell.method()`
- Use `nefu.invoke('notification.method', args)` instead of the `Notification` API
- Use `nefu.invoke('net.method', args)` instead of `electron.net.method()`
- Use `nefu.invoke('webContents.method', args)` instead of `electron.webContents.method()`
- Use `nefu.invoke('session.method', args)` instead of `electron.session.method()`
- Use `nefu.on(event, callback)` instead of `ipcRenderer.on()` for IPC events
- Use `nefu.contextBridge.*` for context bridging to ensure secure isolation

---

> 📝 This manual is written for Nefu v1.0.0 (Rust implementation) and is revised as versions update. If anything is missing or incorrect, feedback is welcome.
>
> © Nefu Project | MIT License
