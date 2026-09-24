//! Nefu - a tool that packages web projects into standalone desktop executables
//!
//! Nefu is a lightweight Web-to-Desktop packaging tool that supports:
//! - The custom .nc (Nefu Coding) declarative UI language
//! - AES-256-GCM encryption to protect source code
//! - A hot-reload dev server
//! - The custom nefu:// protocol
//! - 30+ UI components based on Bootstrap 5

mod android;
mod app_module;
mod auto_updater;
mod bridge;
mod cli;
mod config;
mod content_tracing;
mod context_bridge;
mod crash_reporter;
mod cpp_compiler;
mod dock;
mod global_shortcut;
mod native_image;
mod native_menu;
mod native_theme;
mod nc_parser;
mod notifications;
mod pack;
mod power_save_blocker;
mod process_info;
mod protocol;
mod protocol_handler;
mod rendering;
mod safe_storage;
mod server;
mod session;
mod shell;
mod sql_manager;
mod system_info;
mod system_preferences;
mod threading;
mod utils;
mod web_contents;
mod web_fetcher;
mod webview;

use anyhow::{Context, Result};
use clap::Parser;
use log::{error, info, warn};
use std::path::PathBuf;

/// Nefu application version
const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Main entry point
fn main() {
    // Initialize the logging system
    init_logging();

    // Packaged mode: if there are no CLI arguments and the current executable is a Nefu packaged app,
    // run it directly as a desktop app (artifacts produced by build take this path).
    if !has_cli_arguments() {
        match run_as_packaged_app() {
            Ok(true) => return,          // ran as a packaged app and exited normally
            Ok(false) => { /* not a packaged file, continue with the CLI flow */ }
            Err(e) => {
                error!("failed to run packaged app: {:#}", e);
                std::process::exit(1);
            }
        }
    }

    // Parse the command-line arguments
    let args = cli::Cli::parse();

    // Dispatch based on the subcommand
    let result = match args.command {
        cli::Commands::Start {
            port,
            host,
            no_reload,
            open: _open,
            watch_extensions,
        } => cmd_start(port, host, no_reload, watch_extensions),
        cli::Commands::Build {
            target,
            config,
            installer,
            output_dir,
            no_compress,
            no_encrypt,
            verbose,
            app_name,
            config_overrides,
            apk,
            release,
        } => {
            if target == ".apk" {
                cmd_build_android(app_name, config, config_overrides, output_dir, apk, release)
            } else {
                cmd_build(target, config, installer, output_dir, no_compress, no_encrypt, verbose)
            }
        }
        cli::Commands::Init => cmd_init(),
        cli::Commands::Version => cmd_version(),
        cli::Commands::Cpp { source, output, options } => cmd_cpp(source, output, options),
        cli::Commands::Run {
            name,
            args,
        } => cmd_run(name, args),
    };

    // Unified error handling
    if let Err(e) = result {
        error!("execution failed: {:#}", e);
        std::process::exit(1);
    }
}

/// Check whether any CLI arguments (subcommand) were provided
fn has_cli_arguments() -> bool {
    std::env::args().skip(1).count() > 0
}

/// Run the desktop app in packaged mode
///
/// Unpacks resources from the end of the current executable, reads the config file, and starts the WebView.
/// Returns Ok(false) if the current executable is not a Nefu packaged file.
fn run_as_packaged_app() -> Result<bool> {
    let exe_path = std::env::current_exe().context("failed to get the executable path")?;

    if !pack::is_nefu_package(&exe_path) {
        return Ok(false);
    }

    info!("detected a Nefu packaged app, starting in packaged mode: {}", exe_path.display());

    let mut resource_pack = pack::ResourcePack::load_from_self()
        .context("failed to unpack resources")?;

    info!("successfully unpacked {} resource files", resource_pack.file_count());

    // Read the config from the resource pack
    let nefu_config = {
        let entry_bytes = resource_pack
            .get("main.nefu")
            .cloned()
            .or_else(|| resource_pack.get("nefu.toml").cloned());

        match entry_bytes {
            Some(bytes) => config::NefuConfig::load_from_bytes(&bytes)
                .context("failed to parse the config file in the resource pack")?,
            None => {
                warn!("no config file found in the resource pack, using default config");
                let mut cfg = config::NefuConfig::default();
                cfg.entry = "index.html".to_string();
                cfg
            }
        }
    };

    info!("app name: {}, entry: {}", nefu_config.output, nefu_config.entry);
    info!(
        "window size: {}x{}",
        nefu_config.window_width, nefu_config.window_height
    );

    // Single-instance mode: a failed lock means another instance is already running
    let _instance_lock = if nefu_config.single_instance {
        match acquire_single_instance_lock(&nefu_config) {
            Some(lock) => {
                info!("single-instance mode enabled");
                Some(lock)
            }
            None => {
                error!("the app is already running; single-instance mode forbids a second launch");
                std::process::exit(0);
            }
        }
    } else {
        None
    };

    // Automatic update check (background thread)
    start_update_check(&nefu_config);

    let webview_app = webview::WebViewApp::new_packaged(nefu_config, resource_pack);
    webview_app.run().context("error running the packaged WebView app")?;

    Ok(true)
}

/// Single-instance file lock: a second launch of the same app fails to acquire the lock
///
/// The lock file lives at `.nefu/<output>/instance.lock` under the user data directory,
/// and the returned File must stay alive for the entire process lifetime.
fn acquire_single_instance_lock(config: &config::NefuConfig) -> Option<std::fs::File> {
    let base = user_data_dir();
    let dir = base.join(".nefu").join(&config.output);
    if std::fs::create_dir_all(&dir).is_err() {
        warn!("failed to create the instance lock directory");
        return None;
    }

    let lock_path = dir.join("instance.lock");
    let file = match std::fs::File::create(&lock_path) {
        Ok(f) => f,
        Err(e) => {
            warn!("failed to create the instance lock {}: {}", lock_path.display(), e);
            return None;
        }
    };

    use fs2::FileExt;
    match file.try_lock_exclusive() {
        Ok(()) => {
            info!("single-instance lock acquired: {}", lock_path.display());
            Some(file)
        }
        Err(_) => None,
    }
}

/// Check for updates in the background (when update_url is configured)
///
/// Requests the URL for the remote version (supports JSON `{"version": "x.y.z"}` or plain text),
/// compares it with the local version, and logs when a new version is found.
fn start_update_check(config: &config::NefuConfig) {
    let Some(url) = config.update_url.clone() else {
        return;
    };
    let current = config.version.clone();

    std::thread::spawn(move || {
        let body = match ureq::get(&url).timeout(std::time::Duration::from_secs(10)).call() {
            Ok(resp) => resp.into_string().unwrap_or_default(),
            Err(e) => {
                log::debug!("update check failed: {}", e);
                return;
            }
        };

        let remote = serde_json::from_str::<serde_json::Value>(&body)
            .ok()
            .and_then(|v| v.get("version").and_then(|x| x.as_str()).map(String::from))
            .unwrap_or_else(|| body.trim().to_string());

        if remote.is_empty() {
            return;
        }
        if remote != current {
            info!("new version {} available (current {}), get the update at {}", remote, current, url);
        } else {
            info!("already on the latest version {}", current);
        }
    });
}

/// Get the user data directory
///
/// - Windows: `%APPDATA%`
/// - macOS: `~/Library/Application Support`
/// - Linux: `~/.local/share`
fn user_data_dir() -> std::path::PathBuf {
    #[cfg(windows)]
    {
        if let Ok(appdata) = std::env::var("APPDATA") {
            return std::path::PathBuf::from(appdata);
        }
    }
    #[cfg(target_os = "macos")]
    {
        if let Ok(home) = std::env::var("HOME") {
            return std::path::PathBuf::from(home).join("Library/Application Support");
        }
    }
    #[cfg(target_os = "linux")]
    {
        if let Ok(home) = std::env::var("HOME") {
            return std::path::PathBuf::from(home).join(".local/share");
        }
    }
    std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."))
}

/// Initialize the logging system
///
/// Sets the log output based on the NEFU_LOG environment variable or the default level
fn init_logging() {
    // Try to get the log level from the environment variable, defaulting to info
    let log_level = std::env::var("NEFU_LOG").unwrap_or_else(|_| "info".to_string());

    env_logger::Builder::new()
        .filter_level(match log_level.as_str() {
            "trace" => log::LevelFilter::Trace,
            "debug" => log::LevelFilter::Debug,
            "info" => log::LevelFilter::Info,
            "warn" => log::LevelFilter::Warn,
            "error" => log::LevelFilter::Error,
            _ => log::LevelFilter::Info,
        })
        .format_timestamp_millis()
        .format_module_path(false)
        .init();

    info!("Nefu v{} started", VERSION);
}

/// Start the dev server (with hot reload)
///
/// # Arguments
/// - `port`: the port number
/// - `host`: the listen address
/// - `no_reload`: whether to disable hot reload
/// - `watch_extensions`: extra file extensions to watch
fn cmd_start(
    port: u16,
    host: String,
    no_reload: bool,
    watch_extensions: Vec<String>,
) -> Result<()> {
    // Check required dependencies
    utils::check_required_deps(None);

    info!(
        "starting dev server at: http://{}:{}",
        host, port
    );

    // Find and load the project config
    let project_dir = utils::find_project_dir()
        .context("Nefu project directory not found; make sure a main.nefu config file exists in the current directory")?;

    let config_path = project_dir.join("main.nefu");
    let nefu_config = config::NefuConfig::load(&config_path)
        .context("failed to load the main.nefu config file")?;

    info!("project directory: {}", project_dir.display());
    info!("entry file: {}", nefu_config.entry);

    // Check whether the entry file exists
    let entry_path = project_dir.join(&nefu_config.entry);
    if !entry_path.exists() {
        // If the entry is .html but a same-named .nc file exists, notify the user
        let nc_entry = entry_path.with_extension("nc");
        if nc_entry.exists() {
            warn!(
                "entry file {} does not exist, but a {}.nc file was found; converting automatically",
                nefu_config.entry,
                entry_path
                    .file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
            );
        } else {
            return Err(anyhow::anyhow!(
                "entry file '{}' does not exist in the project directory",
                nefu_config.entry
            ));
        }
    }

    // Start the dev server on a background thread
    let dev_server =
        server::DevServer::new(project_dir.clone(), nefu_config.clone(), port)
            .context("failed to create the dev server")?;

    let server_handle = std::thread::spawn(move || {
        if let Err(e) = dev_server.run() {
            error!("dev server error: {:#}", e);
        }
    });

    // Wait for the server to be ready
    std::thread::sleep(std::time::Duration::from_millis(300));

    // Start the WebView window on the main thread (tao/wry requires windows to be created on the main thread)
    let dev_url = if host == "0.0.0.0" || host.is_empty() {
        format!("http://localhost:{}", port)
    } else {
        format!("http://{}:{}", host, port)
    };
    info!("starting WebView window: {}", dev_url);

    let webview_app = webview::WebViewApp::new_dev(nefu_config, dev_url);

    // no_reload / watch_extensions are parsed in advance for future extensions; the server always provides hot reload for now
    if no_reload {
        warn!("hot reload disabled via --no-reload (the server still injects a reload script; a future version will support turning it off completely)");
    }
    for ext in &watch_extensions {
        info!("extra watched file extension: .{}", ext);
    }

    webview_app.run().context("error running the WebView app")?;

    // After the window closes, wait for the server thread to finish
    let _ = server_handle.join();

    Ok(())
}

/// Build the executable
///
/// # Arguments
/// - `target`: the target platform (exe/app/bin)
/// - `config`: optional config file path
/// - `installer`: whether to generate an installer
/// - `output_dir`: the output directory
/// - `no_compress`: whether to disable resource compression
/// - `no_encrypt`: whether to disable resource encryption
/// - `verbose`: verbose output mode
fn cmd_build(
    target: String,
    config: Option<PathBuf>,
    installer: bool,
    output_dir: Option<PathBuf>,
    no_compress: bool,
    no_encrypt: bool,
    verbose: bool,
) -> Result<()> {
    // Check required dependencies
    utils::check_required_deps(Some(&target));

    // Check for the NSIS required by the installer
    if installer {
        if !utils::check_and_download_dep(utils::DepType::Nsis) {
            log::warn!("NSIS is not installed; the installer may be incomplete");
        }
    }

    // Parse and validate the build arguments
    let build_args = cli::BuildArgs {
        target: target.clone(),
        config: config.clone(),
        installer,
        output_dir,
        no_compress,
        no_encrypt,
        verbose,
    };
    cli::validate_build_args(&build_args)
        .context("invalid build arguments")?;

    // Find the project directory
    let project_dir = utils::find_project_dir()
        .context("Nefu project directory not found")?;

    // Load the config
    let config_path = config.unwrap_or_else(|| project_dir.join("main.nefu"));
    let nefu_config = config::NefuConfig::load(&config_path)
        .context("failed to load the config file")?;

    cli::print_status(
        "INFO",
        &format!(
            "building {} v{} (target: {})",
            nefu_config.output, nefu_config.version, target
        ),
    );

    // Preprocess .nc files into HTML
    cli::print_status("BUILD", "step 1/3: compiling .nc components...");
    preprocess_nc_files(&project_dir)?;

    // Collect all the files to package
    cli::print_status("BUILD", "step 2/3: collecting project files...");
    let files = pack::collect_project_files(&project_dir, &nefu_config)
        .context("failed to collect project files")?;
    cli::print_status("OK", &format!("collected {} project files", files.len()));

    // Determine the output directory and file name
    let out_dir = build_args
        .output_dir
        .clone()
        .unwrap_or_else(|| project_dir.join("dist"));
    let output_name = format!("{}.{}", nefu_config.output, target);
    let output_path = out_dir.join(&output_name);

    // Ensure the output directory exists
    std::fs::create_dir_all(&out_dir)
        .with_context(|| format!("failed to create the output directory: {}", out_dir.display()))?;

    cli::print_status("BUILD", "step 3/3: compressing and encrypting resources...");
    let total_files = files.len();

    // Determine the icon path (if an icon is configured)
    let icon_path = nefu_config.icon.as_ref().map(|icon_name| {
        let icon_abs = project_dir.join(icon_name);
        if icon_abs.exists() {
            cli::print_status("ICON", &format!("using application icon: {}", icon_abs.display()));
            icon_abs
        } else {
            cli::print_status("WARN", &format!("icon file does not exist: {}", icon_abs.display()));
            icon_abs
        }
    });

    pack::build_executable(
        &files,
        &output_path,
        no_compress,
        no_encrypt,
        icon_path.as_deref(),
        Some(&|current, total| cli::print_progress(current, total, "packing resources")),
    )
    .context("failed to build the executable")?;
    if total_files > 0 {
        cli::print_status("OK", &format!("processed {} files", total_files));
    }

    let file_size = std::fs::metadata(&output_path)
        .map(|m| utils::format_size(m.len()))
        .unwrap_or_else(|_| "unknown".to_string());

    // macOS target: reassemble into a standard .app bundle
    let final_path = if target == "app" {
        let bundle = create_macos_app_bundle(&output_path, &nefu_config, icon_path.as_deref())?;
        std::fs::remove_file(&output_path).ok();
        bundle
    } else {
        output_path.clone()
    };

    cli::print_status("OK", &format!("build complete: {}", final_path.display()));
    cli::print_status("INFO", &format!("file size: {}", file_size));

    // Generate the installer if requested (Windows exe only)
    if installer {
        if target != "exe" {
            warn!("--installer currently only supports the Windows exe target; skipped");
        } else {
            cli::print_status("PACK", "generating installer...");
            generate_installer(&output_path, &nefu_config)?;
        }
    }

    Ok(())
}

/// Generate the macOS `.app` bundle structure
///
/// Reorganizes the built executable into a standard bundle:
/// `dist/<Output>.app/Contents/{MacOS/<Output>, Info.plist, Resources/}`
///
/// # Arguments
/// - `binary_path`: the built executable path
/// - `config`: the Nefu config
/// - `icon_src_path`: the app icon source path (optional; automatically copied to the Resources directory)
///
/// # Returns
/// The bundle directory path
fn create_macos_app_bundle(
    binary_path: &std::path::Path,
    config: &config::NefuConfig,
    icon_src_path: Option<&std::path::Path>,
) -> Result<PathBuf> {
    let parent = binary_path.parent().unwrap_or(std::path::Path::new("."));
    let bundle_dir = parent.join(format!("{}.app", config.output));
    let contents_dir = bundle_dir.join("Contents");
    let macos_dir = contents_dir.join("MacOS");
    let resources_dir = contents_dir.join("Resources");

    // Remove an existing .app directory to avoid create_dir_all failures
    if bundle_dir.exists() {
        log::info!("removing existing .app directory: {}", bundle_dir.display());
        std::fs::remove_dir_all(&bundle_dir)
            .with_context(|| format!("failed to remove the existing .app directory: {}", bundle_dir.display()))?;
    }

    std::fs::create_dir_all(&macos_dir)
        .with_context(|| format!("failed to create the .app directory: {}", macos_dir.display()))?;
    std::fs::create_dir_all(&resources_dir)
        .with_context(|| format!("failed to create the Resources directory: {}", resources_dir.display()))?;

    // Copy the executable into Contents/MacOS
    let target_bin = macos_dir.join(&config.output);
    std::fs::copy(binary_path, &target_bin)
        .with_context(|| format!("failed to write the executable: {}", target_bin.display()))?;

    // Set the executable permission
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&target_bin, std::fs::Permissions::from_mode(0o755))
            .with_context(|| format!("failed to set the executable permission: {}", target_bin.display()))?;
    }

    // Copy the app icon into the Resources directory
    if let Some(icon_path) = icon_src_path {
        if icon_path.exists() {
            // On macOS, use the .icns format (or copy the original file)
            let ext = icon_path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
            let dest_icon = if ext == "icns" {
                resources_dir.join("icon.icns")
            } else {
                resources_dir.join("icon.icns")
            };
            std::fs::copy(icon_path, &dest_icon)
                .with_context(|| format!("failed to copy the icon to Resources: {}", icon_path.display()))?;
            log::info!("app icon copied to: {}", dest_icon.display());
        }
    }

    // Generate Info.plist
    let bundle_id = format!("dev.nefu.{}", config.output.to_lowercase());
    let plist = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key>
    <string>{name}</string>
    <key>CFBundleDisplayName</key>
    <string>{name}</string>
    <key>CFBundleIdentifier</key>
    <string>{bundle_id}</string>
    <key>CFBundleVersion</key>
    <string>{version}</string>
    <key>CFBundleShortVersionString</key>
    <string>{version}</string>
    <key>CFBundleExecutable</key>
    <string>{name}</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>LSMinimumSystemVersion</key>
    <string>10.13</string>
    <key>NSHighResolutionCapable</key>
    <true/>
    <key>CFBundleIconFile</key>
    <string>icon</string>
</dict>
</plist>
"#,
        name = config.output,
        bundle_id = bundle_id,
        version = config.version,
    );
    std::fs::write(contents_dir.join("Info.plist"), plist)
        .context("failed to write Info.plist")?;

    info!("macOS app bundle generated: {}", bundle_dir.display());
    Ok(bundle_dir)
}

/// Preprocess all .nc files into HTML
///
/// Scans the project directory for .nc files and converts them to matching .html files
fn preprocess_nc_files(project_dir: &std::path::Path) -> Result<()> {
    use walkdir::WalkDir;

    let mut converted_count = 0;

    for entry in WalkDir::new(project_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path().extension().map_or(false, |ext| ext == "nc")
                && !e.path().starts_with(project_dir.join("dist"))
                && !e.path().starts_with(project_dir.join(".nefu"))
        })
    {
        let nc_path = entry.path();
        let html_path = nc_path.with_extension("html");

        info!(
            "converting NC file: {} -> {}",
            nc_path.file_name().unwrap_or_default().to_string_lossy(),
            html_path.file_name().unwrap_or_default().to_string_lossy()
        );

        let nc_content = std::fs::read_to_string(nc_path)
            .with_context(|| format!("failed to read NC file: {}", nc_path.display()))?;

        let html_content = nc_parser::parse_nc_to_html(&nc_content)
            .with_context(|| format!("failed to parse NC file: {}", nc_path.display()))?;

        std::fs::write(&html_path, html_content)
            .with_context(|| format!("failed to write HTML file: {}", html_path.display()))?;

        converted_count += 1;
    }

    if converted_count > 0 {
        info!("successfully converted {} NC files", converted_count);
    }

    Ok(())
}

/// Initialize a new project
///
/// Interactively guides the user through creating a Nefu project template.
/// Supports two templates: basic (plain HTML/CSS/JS) and nc (declarative UI).
fn cmd_init() -> Result<()> {
    use std::io::{self, Write};

    let current_dir = std::env::current_dir().context("failed to get the current directory")?;

    // Check whether a project already exists
    let has_existing = current_dir.join("main.nefu").exists();
    if has_existing {
        print!("A Nefu project already exists in the current directory. Re-initialize? (y/N): ");
        io::stdout().flush()?;
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let input = input.trim().to_lowercase();
        if input != "y" && input != "yes" {
            info!("initialization cancelled");
            return Ok(());
        }
    }

    info!("initializing Nefu project: {}", current_dir.display());

    // Choose the template type
    println!("\nChoose a template type:");
    println!("  1. Basic  - plain HTML/CSS/JS project (for frontend developers)");
    println!("  2. NC     - declarative .nc UI language project (with built-in Bootstrap 5 components)");
    print!("Choose (1/2, default: 2): ");
    io::stdout().flush()?;
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let use_nc = match input.trim() {
        "1" | "basic" | "Basic" => false,
        _ => true, // default to the NC template
    };

    // Create the main.nefu config file
    let config_content = r#"# Nefu project config file
# Docs: https://nefu.dev/docs/config

# Entry HTML file path
entry = "index.html"

# Output executable name (without extension)
output = "myapp"

# Application description
description = "My Nefu App"

# Application version
version = "1.0.0"

# Author information
author = "Your Name"

# Debug mode (shows developer tools when true)
debug = false

# Preload script path (optional)
# preload = "preload.js"

# Window width (pixels)
window_width = 1024

# Window height (pixels)
window_height = 768

# Whether to start fullscreen
fullscreen = false

# Whether the window is resizable
resizable = true

# Custom User-Agent
user_agent = "nefu/1.0"

# Application icon path (optional)
# icon = "resources/app.ico"
"#;

    std::fs::write(current_dir.join("main.nefu"), config_content)
        .context("failed to create the main.nefu config file")?;

    if use_nc {
        // Create the sample index.nc file (declarative UI)
        let nc_content = r#"{
  "type": "container",
  "props": {
    "class": "py-5"
  },
  "children": [
    {
      "type": "header",
      "props": {
        "class": "text-center mb-4"
      },
      "children": [
        {
          "type": "text",
          "props": {
            "tag": "h1",
            "class": "display-4 fw-bold text-primary"
          },
          "children": ["Welcome to Nefu"]
        },
        {
          "type": "text",
          "props": {
            "tag": "p",
            "class": "lead text-muted"
          },
          "children": ["Package your web project into a standalone desktop app"]
        }
      ]
    },
    {
      "type": "card",
      "props": {
        "class": "shadow-sm"
      },
      "children": [
        {
          "type": "text",
          "props": {
            "tag": "h5",
            "class": "card-header bg-primary text-white"
          },
          "children": ["Quick Start"]
        },
        {
          "type": "container",
          "props": {
            "class": "card-body"
          },
          "children": [
            {
              "type": "text",
              "props": {
                "tag": "p"
              },
              "children": ["Edit this file to build your app UI."]
            },
            {
              "type": "button",
              "props": {
                "class": "btn btn-primary",
                "onclick": "alert('Hello from Nefu!')"
              },
              "children": ["Click me"]
            }
          ]
        }
      ]
    },
    {
      "type": "divider",
      "props": {}
    },
    {
      "type": "footer",
      "props": {
        "class": "text-center text-muted py-3"
      },
      "children": [
        {
          "type": "text",
          "props": {
            "tag": "small"
          },
          "children": ["Powered by Nefu v1.0.0"]
        }
      ]
    }
  ]
}
"#;
        std::fs::write(current_dir.join("index.nc"), nc_content)
            .context("failed to create the index.nc sample file")?;
    } else {
        // Create the basic template (plain HTML/CSS/JS)
        let html_content = r#"<!DOCTYPE html>
<html lang="zh-CN">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>My Nefu App</title>
    <link rel="stylesheet" href="style.css">
</head>
<body>
    <div class="container">
        <header>
            <h1>Welcome to Nefu</h1>
            <p class="subtitle">Package your web project into a standalone desktop app</p>
        </header>

        <main>
            <div class="card">
                <h5 class="card-header">Quick Start</h5>
                <div class="card-body">
                    <p>Edit this file to build your app UI.</p>
                    <button onclick="alert('Hello from Nefu!')">Click me</button>
                </div>
            </div>
        </main>

        <footer>
            <small>Powered by Nefu v1.0.0</small>
        </footer>
    </div>
    <script src="app.js"></script>
</body>
</html>
"#;

        let css_content = r#"* {
    margin: 0;
    padding: 0;
    box-sizing: border-box;
}

body {
    font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
    background: #f5f5f5;
    color: #333;
    min-height: 100vh;
    display: flex;
    justify-content: center;
    align-items: center;
}

.container {
    max-width: 600px;
    width: 100%;
    padding: 2rem;
}

header {
    text-align: center;
    margin-bottom: 2rem;
}

h1 {
    font-size: 2.5rem;
    color: #0d6efd;
    margin-bottom: 0.5rem;
}

.subtitle {
    color: #6c757d;
    font-size: 1.1rem;
}

.card {
    background: white;
    border-radius: 8px;
    box-shadow: 0 2px 8px rgba(0,0,0,0.1);
    overflow: hidden;
}

.card-header {
    background: #0d6efd;
    color: white;
    padding: 0.75rem 1rem;
    font-size: 1rem;
}

.card-body {
    padding: 1.5rem;
}

.card-body p {
    margin-bottom: 1rem;
    color: #495057;
}

button {
    background: #0d6efd;
    color: white;
    border: none;
    padding: 0.5rem 1.5rem;
    border-radius: 4px;
    cursor: pointer;
    font-size: 1rem;
    transition: background 0.2s;
}

button:hover {
    background: #0b5ed7;
}

footer {
    text-align: center;
    margin-top: 2rem;
    color: #6c757d;
}
"#;

        let js_content = r#"// Nefu app main script
console.log('Nefu app started');

// Send data to the Rust side via the lj() function
// lj({ type: 'app_ready', timestamp: Date.now() });

// Listen for the nefu-ready event
window.addEventListener('nefu-ready', function() {
    console.log('Nefu Bridge ready');
    // Example: get app info
    // nefu.getInfo().then(info => console.log(info));
});
"#;

        std::fs::write(current_dir.join("index.html"), html_content)
            .context("failed to create the index.html file")?;
        std::fs::write(current_dir.join("style.css"), css_content)
            .context("failed to create the style.css file")?;
        std::fs::write(current_dir.join("app.js"), js_content)
            .context("failed to create the app.js file")?;
    }

    // Create the resources directory
    let resources_dir = current_dir.join("resources");
    std::fs::create_dir_all(&resources_dir)
        .context("failed to create the resources directory")?;

    // Create the .gitignore
    let gitignore_content = "# Nefu build artifacts\n/dist/\n/.nefu/\n*.exe\n*.app\n*.bin\n\n# Editors\n.idea/\n.vscode/\n*.swp\n*.swo\n\n# System files\n.DS_Store\nThumbs.db\n";
    std::fs::write(current_dir.join(".gitignore"), gitignore_content)
        .context("failed to create the .gitignore file")?;

    info!("project initialized!");
    info!("");
    info!("created the following files:");
    if use_nc {
        info!("  main.nefu    - project config file");
        info!("  index.nc     - sample page (NC format)");
        info!("  resources/   - resource files directory");
        info!("  .gitignore   - Git ignore rules");
        info!("");
        info!("next steps:");
        info!("  1. edit index.nc to design your UI");
        info!("  2. run 'nefu start' to start the dev server");
        info!("  3. run 'nefu build exe' to build the executable");
    } else {
        info!("  main.nefu    - project config file");
        info!("  index.html   - sample page");
        info!("  style.css    - stylesheet");
        info!("  app.js       - JavaScript script");
        info!("  resources/   - resource files directory");
        info!("  .gitignore   - Git ignore rules");
        info!("");
        info!("next steps:");
        info!("  1. edit index.html to design your UI");
        info!("  2. run 'nefu start' to start the dev server");
        info!("  3. run 'nefu build exe' to build the executable");
    }

    Ok(())
}

/// Run a script defined in the project
///
/// Looks up the script name in the [scripts] section of main.nefu,
/// then runs the corresponding shell command in the project directory.
///
/// # Arguments
/// - `name`: the script name
/// - `args`: extra arguments passed to the script
fn cmd_run(name: String, args: Vec<String>) -> Result<()> {
    let project_dir = utils::find_project_dir()
        .context("Nefu project directory not found")?;

    let config_path = project_dir.join("main.nefu");
    let nefu_config = config::NefuConfig::load(&config_path)
        .context("failed to load the main.nefu config file")?;

    // Look up the script
    let command = nefu_config.scripts.get(&name).cloned().ok_or_else(|| {
        let available: Vec<String> = nefu_config.scripts.keys().cloned().collect();
        anyhow::anyhow!(
            "script '{}' not found.\navailable scripts: {}",
            name,
            if available.is_empty() {
                "(none)".to_string()
            } else {
                available.join(", ")
            }
        )
    })?;

    info!("running script '{}': {}", name, command);

    // Build the shell command
    let full_command = if args.is_empty() {
        command.clone()
    } else {
        format!("{} {}", command, args.join(" "))
    };

    #[cfg(windows)]
    {
        // On Windows, run via cmd /c
        let status = std::process::Command::new("cmd")
            .args(["/C", &full_command])
            .current_dir(&project_dir)
            .stdin(std::process::Stdio::inherit())
            .stdout(std::process::Stdio::inherit())
            .stderr(std::process::Stdio::inherit())
            .status()
            .context("failed to run the script")?;

        if !status.success() {
            if let Some(code) = status.code() {
                anyhow::bail!("script '{}' exited with status code: {}", name, code);
            } else {
                anyhow::bail!("script '{}' terminated by a signal", name);
            }
        }
    }

    #[cfg(not(windows))]
    {
        // On Unix, run via sh -c
        let status = std::process::Command::new("sh")
            .args(["-c", &full_command])
            .current_dir(&project_dir)
            .stdin(std::process::Stdio::inherit())
            .stdout(std::process::Stdio::inherit())
            .stderr(std::process::Stdio::inherit())
            .status()
            .context("failed to run the script")?;

        if !status.success() {
            if let Some(code) = status.code() {
                anyhow::bail!("script '{}' exited with status code: {}", name, code);
            } else {
                anyhow::bail!("script '{}' terminated by a signal", name);
            }
        }
    }

    info!("script '{}' finished", name);
    Ok(())
}

/// Show version information
fn cmd_version() -> Result<()> {
    println!("Nefu v{}", VERSION);
    println!("Web-to-Desktop packaging tool");
    println!("");
    println!("features:");
    println!("  • Custom .nc declarative UI language");
    println!("  • AES-256-GCM source encryption");
    println!("  • Hot-reload dev server");
    println!("  • Custom nefu:// protocol");
    println!("  • 30+ Bootstrap 5 UI components");
    println!("  • C/C++ compilation support");
    println!("  • Android APK build support");
    Ok(())
}

/// Compile a C/C++ source file
///
/// # Arguments
/// - `source`: the source file path
/// - `output`: optional output file path
/// - `options`: extra compile options
fn cmd_cpp(source: String, output: Option<PathBuf>, options: Vec<String>) -> Result<()> {
    let source_path = PathBuf::from(&source);
    let source_abs = if source_path.is_absolute() {
        source_path
    } else {
        std::env::current_dir()
            .context("failed to get the current directory")?
            .join(&source_path)
    };

    cli::print_status("INFO", &format!("compiling C/C++ source file: {}", source_abs.display()));

    // Check whether the source file exists
    if !source_abs.exists() {
        anyhow::bail!("source file does not exist: {}", source_abs.display());
    }

    // Detect the file type
    let ext = source_abs
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();

    if ext != "c" && ext != "cpp" {
        anyhow::bail!(
            "unsupported file type '.{}'; only .c and .cpp files are supported",
            ext
        );
    }

    // Determine the output path
    let output_path = match output {
        Some(p) => {
            if p.is_absolute() { p } else {
                std::env::current_dir().context("failed to get the current directory")?.join(p)
            }
        }
        None => {
            let stem = source_abs.file_stem().unwrap_or_default();
            #[cfg(windows)]
            {
                source_abs.with_file_name(format!("{}.exe", stem.to_string_lossy()))
            }
            #[cfg(not(windows))]
            {
                source_abs.with_file_name(stem)
            }
        }
    };

    cli::print_status("BUILD", &format!("compiling..."));
    cli::print_status("INFO", &format!("output file: {}", output_path.display()));

    let result = cpp_compiler::compile(&source_abs, Some(&output_path), &options)?;

    let file_size = std::fs::metadata(&result)
        .map(|m| utils::format_size(m.len()))
        .unwrap_or_else(|_| "unknown".to_string());

    cli::print_status("OK", &format!("compilation succeeded: {}", result.display()));
    cli::print_status("INFO", &format!("file size: {}", file_size));

    Ok(())
}

/// Generate a Windows installer (NSIS)
///
/// 1. Generates the `<output>_installer.nsi` script in the output directory
/// 2. If NSIS is installed (`makensis` is in PATH), automatically compiles `<output>_setup.exe`
/// 3. Otherwise prompts the user to install NSIS and compile manually
///
/// # Arguments
/// - `executable_path`: the built executable path
/// - `config`: the project config
fn generate_installer(
    executable_path: &std::path::Path,
    config: &config::NefuConfig,
) -> Result<()> {
    let out_dir = executable_path
        .parent()
        .unwrap_or(std::path::Path::new("."));
    let exe_name = executable_path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| format!("{}.exe", config.output));
    let nsi_path = out_dir.join(format!("{}_installer.nsi", config.output));
    let title = config.get_title();

    // Generate the NSIS script
    let nsi = format!(
        r#"; Nefu installer script -- automatically generated by `nefu build exe --installer`
; Requires makensis from NSIS (https://nsis.sourceforge.io) to compile
Name "{title}"
OutFile "{output}_setup.exe"
InstallDir "$PROGRAMFILES64\{output}"
InstallDirRegKey HKLM "Software\{output}" "InstallDir"
RequestExecutionLevel admin

Page directory
Page instfiles
UninstPage uninstConfirm
UninstPage instfiles

Section "Install"
  SetOutPath "$INSTDIR"
  File "{exe_name}"
  WriteUninstaller "$INSTDIR\uninstall.exe"
  CreateShortcut "$DESKTOP\{title}.lnk" "$INSTDIR\{exe_name}"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\{output}" "DisplayName" "{title}"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\{output}" "DisplayVersion" "{version}"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\{output}" "UninstallString" '"$INSTDIR\uninstall.exe"'
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\{output}" "InstallDir" "$INSTDIR"
SectionEnd

Section "Uninstall"
  Delete "$INSTDIR\{exe_name}"
  Delete "$INSTDIR\uninstall.exe"
  Delete "$DESKTOP\{title}.lnk"
  RMDir "$INSTDIR"
  DeleteRegKey HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\{output}"
  DeleteRegKey HKLM "Software\{output}"
SectionEnd
"#,
        title = title,
        output = config.output,
        version = config.version,
        exe_name = exe_name,
    );

    std::fs::write(&nsi_path, nsi)
        .with_context(|| format!("failed to write the installer script: {}", nsi_path.display()))?;

    info!("NSIS installer script generated: {}", nsi_path.display());

    // Try to compile with makensis
    match std::process::Command::new("makensis").arg(&nsi_path).status() {
        Ok(status) if status.success() => {
            cli::print_status(
                "OK",
                &format!("installer generated: {}", out_dir.join(format!("{}_setup.exe", config.output)).display()),
            );
        }
        Ok(_) => {
            warn!(
                "makensis compilation failed; you can run it manually: makensis \"{}\"",
                nsi_path.display()
            );
        }
        Err(_) => {
            warn!(
                "makensis not found; install NSIS and run manually: makensis \"{}\"",
                nsi_path.display()
            );
        }
    }

    Ok(())
}

/// Generate an Android project template (optionally build the APK in one step)
///
/// # Arguments
/// - `app_name`: the application name
/// - `config`: optional config file path
/// - `output_dir`: the output directory
/// - `build_apk`: whether to build the APK automatically (fully automated: downloads JDK/SDK/Gradle + builds)
/// - `release`: whether to build a release version (defaults to debug)
fn cmd_build_android(
    app_name: String,
    config: Option<PathBuf>,
    config_overrides: Vec<String>,
    output_dir: Option<PathBuf>,
    build_apk: bool,
    release: bool,
) -> Result<()> {
    cli::print_status("INFO", &format!("preparing to build Android project: {}", app_name));

    // Find the project directory
    let project_dir = utils::find_project_dir()
        .context("Nefu project directory not found")?;

    // Load the config
    let config_path = config.unwrap_or_else(|| project_dir.join("main.nefu"));
    let mut nefu_config = config::NefuConfig::load(&config_path)
        .context("failed to load the config file")?;

    // Apply config overrides (-s key=value)
    for override_str in &config_overrides {
        if let Some(eq_pos) = override_str.find('=') {
            let key = &override_str[..eq_pos];
            let value = &override_str[eq_pos + 1..];
            cli::print_status("CONFIG", &format!("override config: {} = {}", key, value));
            match key {
                "entry" => nefu_config.entry = value.to_string(),
                "output" => nefu_config.output = value.to_string(),
                "description" => nefu_config.description = value.to_string(),
                "version" => nefu_config.version = value.to_string(),
                "author" => nefu_config.author = value.to_string(),
                "debug" => nefu_config.debug = value.parse().unwrap_or(false),
                "window_width" => nefu_config.window_width = value.parse().unwrap_or(nefu_config.window_width),
                "window_height" => nefu_config.window_height = value.parse().unwrap_or(nefu_config.window_height),
                "fullscreen" => nefu_config.fullscreen = value.parse().unwrap_or(false),
                "resizable" => nefu_config.resizable = value.parse().unwrap_or(true),
                "decorations" => nefu_config.decorations = value.parse().unwrap_or(true),
                "always_on_top" => nefu_config.always_on_top = value.parse().unwrap_or(false),
                "single_instance" => nefu_config.single_instance = value.parse().unwrap_or(false),
                "persist_window_state" => nefu_config.persist_window_state = value.parse().unwrap_or(true),
                "context_menu" => nefu_config.context_menu = value.parse().unwrap_or(true),
                "drag_drop" => nefu_config.drag_drop = value.parse().unwrap_or(false),
                "render_optimization" => nefu_config.render_optimization = value.parse().unwrap_or(true),
                "rendering_mode" => {
                    match value {
                        "auto" | "high-performance" | "power-saving" => nefu_config.rendering_mode = value.to_string(),
                        _ => cli::print_status("WARN", &format!("invalid rendering mode: {}, using default 'auto'", value)),
                    }
                }
                "icon" => nefu_config.icon = Some(value.to_string()),
                "title" => nefu_config.title = Some(value.to_string()),
                "csp" => nefu_config.csp = Some(value.to_string()),
                "protocol" => nefu_config.protocol = value.to_string(),
                "update_url" => nefu_config.update_url = Some(value.to_string()),
                _ => cli::print_status("WARN", &format!("unknown config key: {}, ignored", key)),
            }
        } else {
            cli::print_status("WARN", &format!("invalid config override format: {} (expected key=value)", override_str));
        }
    }

    // Preprocess the .nc files
    cli::print_status("BUILD", "compiling .nc components...");
    preprocess_nc_files(&project_dir)?;

    // Determine the output directory
    let out_dir = output_dir.unwrap_or_else(|| project_dir.join("android"));

    cli::print_status("BUILD", &format!("generating Android project at: {}", out_dir.display()));

    // Use the output name from the config (if the user kept the default)
    let final_app_name = if app_name == "NefuApp" {
        nefu_config.output.clone()
    } else {
        app_name
    };

    android::generate_android_project(&project_dir, &out_dir, &final_app_name, &nefu_config.entry)
        .context("failed to generate the Android project")?;

    cli::print_status("OK", &format!("Android project generated: {}", out_dir.display()));

    if build_apk {
        cli::print_status("BUILD", "starting fully automated APK build...");
        cli::print_status("INFO", "JDK, Android SDK, and Gradle will be downloaded automatically (first time only; cached afterwards)");

        match android::build_apk(&out_dir) {
            Ok(apk_path) => {
                let apk_size = std::fs::metadata(&apk_path)
                    .map(|m| m.len())
                    .unwrap_or(0);
                let size_mb = apk_size as f64 / 1024.0 / 1024.0;

                cli::print_status("OK", &format!("APK built successfully!"));
                cli::print_status("INFO", &format!("output file: {}", apk_path.display()));
                cli::print_status("INFO", &format!("file size: {:.2} MB", size_mb));

                // Copy to the project dist directory
                let dist_dir = project_dir.join("dist");
                std::fs::create_dir_all(&dist_dir)?;
                let dist_apk = dist_dir.join(format!("{}.apk", final_app_name));
                std::fs::copy(&apk_path, &dist_apk)?;
                cli::print_status("OK", &format!("copied to: {}", dist_apk.display()));
            }
            Err(e) => {
                cli::print_status("ERROR", &format!("APK build failed: {}", e));
                cli::print_status("INFO", "the Android project has been generated; you can open and build it manually with Android Studio");
                cli::print_status("INFO", &format!("project directory: {}", out_dir.display()));
                return Err(e);
            }
        }
    } else {
        cli::print_status("INFO", "use the --apk option to build the APK automatically in one step (auto-downloads JDK/Android SDK/Gradle)");
        cli::print_status("INFO", &format!("or open {} in Android Studio to build manually", out_dir.display()));
    }

    let _ = release; // release build reserved for the future; currently defaults to debug
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_constant() {
        assert!(!VERSION.is_empty());
    }
}
