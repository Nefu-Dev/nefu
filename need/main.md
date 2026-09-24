# Nefu Project Complete Prompt Summary

## 📋 Original Requirements Evolution

### Phase 1: Basic Requirements
**User Request**: Build an application that converts web files into executable files
- Run `nefu start` to start the project
- The project structure includes a `main.nefu` config file and a `main.js` startup script
- Support `nefu build {ext}` to build executables for different platforms
- Implemented with multiple files using Rust + Go
- Support `preload` and the `lj()` communication function

### Phase 2: Cross-Platform Complete Implementation
**User Request**: A cross-platform, multi-file implementation without bugs
- Provide complete implementations in both Rust and Go
- All code must compile and run
- Support Windows, macOS, and Linux

### Phase 3: Enhanced Features
**User Request**: Add more and more complex features that work on all platforms for production use
- Production-grade features: full config parsing, hot reload, bidirectional communication, logging system
- Support preload script injection
- Custom protocol `nefu://`
- System tray, window state persistence
- 600+ lines per core file

### Phase 4: Specific Resource Packaging
**User Request**: app.gk is a binary web-page archive that prevents decompilation
- Support `resources/app.gk` (binary file)
- Support `language/chinese.k2` and `english.k2` language files
- AES-256 encrypted packaging
- Anti-decompilation mechanism

### Phase 5: Tooling
**User Request**: How to package the entire Rust project as an exe so it can be copied into any project anywhere
- Compile into a standalone `nefu.exe` tool
- Can be copied to any project root directory for use
- Automatically package all files in the current directory

### Phase 6: .nc Language Support
**User Request**: Add more, make the features complete — include basically all components found on pages in the market
- Create a new `.nc` (Nefu Coding) language
- 30+ built-in components (buttons, tables, cards, modals, navbars, etc.)
- Rendered with Bootstrap 5
- No need to write HTML/CSS/JS

### Phase 7: Documentation and Website
**User Request**: Provide a Markdown manual covering all usage methods
**User Request**: Create an official website for it

---

## 🎯 Final Product Complete Feature List

### 1. Core Features
- ✅ Package web projects into standalone executables
- ✅ AES-256-GCM encrypt all resources
- ✅ Support traditional HTML projects
- ✅ Support the `.nc` declarative UI language
- ✅ Development mode (hot reload)
- ✅ Production mode (encrypted packaging)

### 2. Supported Input Formats
- **HTML project**: `index.html` + related resources
- **.nc project**: `index.nc` JSON description file
- **Mixed mode**: .nc is automatically converted to HTML

### 3. Output Formats
- Windows: `.exe` (optional NSIS installer)
- macOS: `.app`
- Linux: `.bin`

### 4. .nc Language Component Library (30+)
| Component | Description |
|------|------|
| button | Button (multiple styles, sizes) |
| input | Input field (multiple types) |
| textarea | Multi-line text area |
| select | Dropdown select |
| checkbox | Checkbox |
| radio | Radio button |
| table | Table |
| card | Card |
| alert | Alert box |
| modal | Modal dialog |
| navbar | Navigation bar |
| container | Container |
| row | Grid row |
| col | Grid column |
| text | Text (multiple tags) |
| image | Image |
| badge | Badge |
| progress | Progress bar |
| spinner | Loading animation |

### 5. Configuration Options (main.nefu)
```toml
entry = "index.html"          # Entry file
output = "myapp"              # Output file name
description = "My App"        # Application description
version = "1.0.0"             # Version number
author = "Your Name"          # Author
no debug = false              # Disable debug
preload = "preload.js"        # Preload script
window_width = 1024           # Window width
window_height = 768           # Window height
fullscreen = false            # Fullscreen
resizable = true              # Resizable
user_agent = "nefu/1.0"       # User-Agent
icon = "resources/app.ico"    # Icon
```

### 6. Security Features
- AES-256 encrypt all resources
- Resources exist only in memory (never written to disk)
- SHA256 checksum verification
- Magic number `NEFUPACK` marker

### 7. Developer Experience
- Hot reload (auto refresh on file changes)
- Dev server (HTTP + WebSocket)
- Logging system (with levels)
- Friendly error messages

---

## 📁 Project File Structure

### Rust Project Files
```
nefu/
├── Cargo.toml              # Dependency config
├── build.rs                # Build script (icon embedding)
└── src/
    ├── main.rs             # Entry point
    ├── cli.rs              # CLI parsing
    ├── config.rs           # Config parsing
    ├── pack.rs             # Encrypted packaging
    ├── webview.rs          # WebView management
    ├── server.rs           # Dev server
    ├── protocol.rs         # Custom protocol
    ├── bridge.rs           # JS bridge
    ├── nc_parser.rs        # .nc parsing
    └── utils.rs            # Utility functions
```

### Website Files
```
website/
└── index.html              # Website homepage (single file)
```

---

## 🛠️ Tech Stack

### Rust Dependencies
```toml
clap = "4.5"        # CLI parsing
wry = "0.45"        # WebView rendering
tao = "0.30"        # Window management
serde = "1.0"       # Serialization
toml = "0.8"        # TOML parsing
anyhow = "1.0"      # Error handling
walkdir = "2.5"     # Directory traversal
zip = "2.0"         # ZIP compression
log = "0.4"         # Logging
env_logger = "0.11" # Log output
notify = "6.1"      # File watching
mime_guess = "2.0"  # MIME types
tiny_http = "0.12"  # HTTP server
ws = "0.10"         # WebSocket
sha2 = "0.10"       # SHA256
lru = "0.12"        # LRU cache
aes-gcm = "0.10"    # AES encryption
rand = "0.8"        # Random numbers
tray-item = "0.7"   # System tray
winres = "0.1"      # Windows resources
```

### Frontend (auto-generated)
- Bootstrap 5.3.0
- Font Awesome 6.5.0
- Inter font

---

## 📚 Command Reference

### Development Commands
```bash
# Build the Nefu tool
cargo build --release

# Development preview (hot reload)
nefu start
nefu start --port 8080

# Package the app
nefu build .exe
nefu build .exe --config main.nefu
nefu build .exe --installer
```

### Packaging Flow
1. Read the config (`main.nefu`)
2. Detect the entry (`.nc` is automatically converted to HTML)
3. Collect all files (excluding itself)
4. ZIP compression
5. AES-256 encryption
6. Compute SHA256
7. Append to the end of the EXE
8. (Optional) Generate NSIS installer

### Runtime Flow
1. Open its own file
2. Read the trailing magic number `NEFUPACK`
3. Read the data length and checksum
4. Read the encrypted data
5. AES-256 decryption
6. Decompress the ZIP into memory
7. Launch the WebView
8. Serve resources via the `nefu://` protocol

---

## 🔒 Security Mechanism Details

### Encryption Structure
```
┌─────────────────────┐
│   nefu executable   │
├─────────────────────┤
│  AES-256 encrypted  │  ← all resources encrypted
│        ZIP          │
├─────────────────────┤
│   8 bytes: length   │  ← Little Endian
├─────────────────────┤
│   32 bytes: SHA256  │  ← checksum
├─────────────────────┤
│ 8 bytes: "NEFUPACK" │  ← magic number
└─────────────────────┘
```

### Encryption Flow
1. ZIP-compress all resources
2. Generate a random Nonce (12 bytes)
3. AES-256-GCM encrypt
4. Concatenate Nonce + Ciphertext
5. Compute the SHA256 checksum
6. Append to the end of the EXE

### Decryption Flow
1. Verify the magic number
2. Read the data length
3. Extract the encrypted data
4. Separate Nonce and Ciphertext
5. AES-256-GCM decrypt
6. Verify SHA256
7. Decompress the ZIP into memory

---

## 🌐 Website Design

### Page Structure
1. **Navbar** - Logo + nav links + download button
2. **Hero** - headline + description + CTA buttons
3. **Feature showcase** - 6 core feature cards
4. **Quick start** - step instructions + command examples
5. **Code samples** - .nc code + rendered preview
6. **Download area** - links for each platform + GitHub link
7. **Footer** - social links + copyright info

### Design Style
- Modern minimal (Inter font)
- Blue theme (#3b82f6)
- Card-based layout
- Responsive design
- Smooth scrolling

---

## 📝 Use Cases

### Use Case 1: Traditional Web to Desktop App
```
Project directory/
├── index.html
├── style.css
├── main.js
└── images/
    └── logo.png

Command: nefu build .exe
Output: myapp.exe
```

### Use Case 2: Quickly Build with .nc
```
Project directory/
└── index.nc

Command: nefu build .exe
Output: myapp.exe (.nc → HTML auto-converted)
```

### Use Case 3: Multilingual App
```
Project directory/
├── index.nc
├── language/
│   ├── chinese.k2
│   └── english.k2
└── resources/
    └── app.gk

Command: nefu build .exe
Output: myapp.exe (all resources encrypted into package)
```

### Use Case 4: Development & Debugging
```
Project directory/
├── index.html
└── main.js

Command: nefu start
Effect: window opens automatically, files refresh instantly on change
```

---

## 🚀 Future Extension Directions

1. **More platform support**
   - Android (via WebView)
   - iOS (via WKWebView)

2. **More components**
   - Chart components (Chart.js)
   - Rich text editor
   - Date picker
   - File upload

3. **Enhanced features**
   - Auto-update mechanism
   - Plugin system
   - Multi-window support
   - Database integration (SQLite)

4. **Optimizations**
   - Smaller size (strip optimization)
   - Faster startup
   - Better error recovery

---

## 📖 Complete Prompt Overview

### Core Prompt
```
Build an application that converts web files into executable files:
- After running nefu start, the project structure is a main.nefu config file and a main.js startup script
- Support nefu build {ext} for different platforms
- Implemented with multiple files using Rust + Go
- Support preload and the lj() communication function
```

### Enhancement Prompt
```
Add more and more complex features that work on all platforms for production use
- Full config parsing, hot reload, bidirectional communication, logging system
- System tray, window state persistence
- 600+ lines per core file
```

### Encryption Prompt
```
app.gk is a binary web-page archive that prevents decompilation
- Support resources/app.gk, language/chinese.k2, language/english.k2
- AES-256 encryption
- Anti-decompilation mechanism
```

### Tooling Prompt
```
How to package the entire Rust project as an exe so it can be copied into any project anywhere
- Compile into a standalone nefu.exe
- Can be copied to any project directory for use
```

### .nc Language Prompt
```
Add more, make the features complete — include basically all components found on pages in the market
- Create the .nc (Nefu Coding) language
- 30+ built-in components
- Based on Bootstrap 5
```

### Documentation Prompt
```
Provide a Markdown manual covering all usage methods so it can be applied to my code
```
```
Create an official website for it
```

---

## 🎯 Final Deliverables Checklist

1. ✅ **Rust source code** (12 files, fully compilable)
2. ✅ **Go source code** (fully compilable)
3. ✅ **Build guide** (install, build, use)
4. ✅ **Config file example** (main.nefu)
5. ✅ **.nc language spec** (JSON format definition)
6. ✅ **Usage tutorial** (develop, package, distribute)
7. ✅ **Markdown manual** (full documentation)
8. ✅ **Website HTML** (single file, ready to deploy)
9. ✅ **Sample code** (.nc demo)
10. ✅ **FAQ**

---

This concludes the complete prompt summary for the Nefu project, covering the entire evolution from initial requirements to final delivery, including technical details and feature highlights. You can use this summary to quickly review the project's design rationale and implementation approach.
