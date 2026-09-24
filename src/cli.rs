//! Nefu command-line interface module
//!
//! Uses clap derive macros to define command-line argument parsing, supporting the following subcommands:
//! - `start`: start the development server (with hot reload)
//! - `build`: build executables / APK
//! - `cpp`: compile C/C++ source files
//! - `init`: initialize a new project
//! - `run`: run scripts
//! - `version`: show version information

use clap::{Parser, Subcommand, Args};
use std::path::PathBuf;

/// Nefu - a tool that packages web projects into standalone desktop executables
///
/// Nefu supports the custom .nc declarative UI language, AES-256-GCM encryption,
/// a hot-reload development server, and the custom nefu:// protocol.
#[derive(Parser, Debug)]
#[command(
    name = "nefu",
    version,
    about = "Packages web projects into standalone desktop executables",
    long_about = r#"
Nefu - Web-to-Desktop packaging tool

Packages a complete web project (HTML/CSS/JS) into a single standalone desktop executable.
Supports the custom .nc (Nefu Coding) declarative UI language, with 30+ built-in Bootstrap 5 components.

Examples:
  nefu init              Initialize a new project
  nefu start             Start the development server
  nefu start --port 8080 Start the server on a specific port
  nefu build exe         Build a Windows executable
  nefu build app         Build a macOS app
  nefu build bin         Build a Linux executable
  nefu build .apk        Build an Android APK
  nefu cpp main.cpp      Compile C/C++ source files
"#
)]
pub struct Cli {
    /// Subcommands
    #[command(subcommand)]
    pub command: Commands,
}

/// Subcommands supported by Nefu
#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Start the development server (with hot reload)
    ///
    /// Starts a local HTTP server with WebSocket hot-reload support.
    /// The browser automatically refreshes after .nc or HTML files change.
    #[command(name = "start")]
    Start {
        /// Server port number (default 3000)
        #[arg(short, long, default_value_t = 3000)]
        port: u16,

        /// Server bind address (default 127.0.0.1)
        #[arg(long, default_value = "127.0.0.1")]
        host: String,

        /// Disable hot reload
        #[arg(long, default_value_t = false)]
        no_reload: bool,

        /// Automatically open the system browser (enabled by default)
        #[arg(long, default_value_t = true)]
        open: bool,

        /// Additional file extensions to watch (comma-separated)
        #[arg(long, value_delimiter = ',')]
        watch_extensions: Vec<String>,
    },

    /// Build an executable file or APK
    ///
    /// Supports building desktop targets (exe/app/bin) and Android (.apk).
    /// All resource files are encrypted with AES-256-GCM and embedded into the executable.
    ///
    /// Examples:
    ///   nefu build exe         Build a Windows executable
    ///   nefu build app         Build a macOS app
    ///   nefu build bin         Build a Linux executable
    ///   nefu build .apk        Build an Android APK
    ///   nefu build .apk -c custom.nefu -s debug=true
    #[command(name = "build")]
    Build {
        /// Target platform format: exe (Windows), app (macOS), bin (Linux), .apk (Android)
        #[arg(value_name = "TARGET")]
        target: String,

        /// Configuration file path (defaults to main.nefu)
        #[arg(short, long)]
        config: Option<PathBuf>,

        /// Also generate an installer (exe target only)
        #[arg(long)]
        installer: bool,

        /// Output directory (defaults to ./dist or ./android)
        #[arg(short, long)]
        output_dir: Option<PathBuf>,

        /// Do not compress resource files (faster builds, larger files)
        #[arg(long, default_value_t = false)]
        no_compress: bool,

        /// Do not encrypt resource files (debugging only)
        #[arg(long, default_value_t = false)]
        no_encrypt: bool,

        /// Verbose output mode
        #[arg(short, long, default_value_t = false)]
        verbose: bool,

        /// Application name (only for .apk targets)
        #[arg(short = 'n', long = "name", default_value = "NefuApp")]
        app_name: String,

        /// Configuration overrides (only for .apk targets, repeatable, format: key=value)
        /// Example: -s entry=index.html -s window_width=1280 -s debug=true
        #[arg(short = 's', long = "set", default_values_t = Vec::<String>::new())]
        config_overrides: Vec<String>,

        /// Fully automated APK build (only for .apk targets; automatically downloads JDK/Android SDK/Gradle)
        #[arg(long, default_value_t = false)]
        apk: bool,

        /// Build a release version (default debug, only for .apk targets)
        #[arg(long, default_value_t = false)]
        release: bool,
    },

    /// Compile C/C++ source files
    ///
    /// Compiles source files using the system-installed C/C++ compiler (gcc/g++/clang).
    /// Supports .c and .cpp files, outputting an executable with the same name.
    ///
    /// Examples:
    ///   nefu cpp main.cpp              Compile main.cpp -> main.exe
    ///   nefu cpp main.c -o app.exe     Compile main.c -> app.exe
    ///   nefu cpp main.cpp -O2 -std=c++17  With compile options
    #[command(name = "cpp")]
    Cpp {
        /// C/C++ source file path
        source: String,

        /// Output file path (defaults to the source file name without extension)
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Extra compile options (e.g. -O2, -std=c++17)
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        options: Vec<String>,
    },

    /// Initialize a new Nefu project
    ///
    /// Creates a project template in the current directory, including the main.nefu
    /// configuration file and an index.nc example page.
    #[command(name = "init")]
    Init,

    /// Show version information
    #[command(name = "version")]
    Version,

    /// Run scripts defined in the project
    ///
    /// Executes commands defined in the [scripts] section of the main.nefu configuration file.
    /// Supports .cmd/.bat/.ps1 script files or any shell command.
    ///
    /// Examples:
    ///   nefu run build-css      Run the configured build-css script
    ///   nefu run deploy         Run the deploy script
    #[command(name = "run")]
    Run {
        /// Name of the script to run (defined in [scripts] in main.nefu)
        name: String,

        /// Extra arguments passed to the script
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
}

/// Detailed arguments for the build command
#[derive(Args, Debug, Clone)]
pub struct BuildArgs {
    /// Target platform format
    #[arg(value_name = "TARGET")]
    pub target: String,

    /// Custom configuration file path
    #[arg(short, long)]
    pub config: Option<PathBuf>,

    /// Whether to generate an installer
    #[arg(long, default_value_t = false)]
    pub installer: bool,

    /// Output directory (defaults to ./dist)
    #[arg(short, long)]
    pub output_dir: Option<PathBuf>,

    /// Do not compress resource files (faster builds, larger files)
    #[arg(long, default_value_t = false)]
    pub no_compress: bool,

    /// Do not encrypt resource files (debugging only)
    #[arg(long, default_value_t = false)]
    pub no_encrypt: bool,

    /// Verbose output mode
    #[arg(short, long, default_value_t = false)]
    pub verbose: bool,
}

/// Detailed arguments for the start command
#[derive(Args, Debug, Clone)]
pub struct StartArgs {
    /// Server listening port
    #[arg(short, long, default_value_t = 3000)]
    pub port: u16,

    /// Server bind address
    #[arg(long, default_value = "127.0.0.1")]
    pub host: String,

    /// Disable hot reload
    #[arg(long, default_value_t = false)]
    pub no_reload: bool,

    /// Automatically open the browser
    #[arg(long, default_value_t = true)]
    pub open: bool,

    /// Watch additional file extensions
    #[arg(long, value_delimiter = ',')]
    pub watch_extensions: Vec<String>,
}

/// Validate whether a target platform string is valid
///
/// # Parameters
/// - `target`: target platform identifier
///
/// # Returns
/// Ok(()) if the target platform is valid, otherwise an error description
pub fn validate_target(target: &str) -> Result<(), String> {
    match target {
        "exe" | "app" | "bin" | ".apk" => Ok(()),
        _ => Err(format!(
            "unsupported target platform '{}'. Supported formats: exe, app, bin, .apk",
            target
        )),
    }
}

/// Get the file extension for a target platform
///
/// # Parameters
/// - `target`: target platform identifier
///
/// # Returns
/// The corresponding file extension string
pub fn target_extension(target: &str) -> &'static str {
    match target {
        "exe" => "exe",
        "app" => "app",
        "bin" => "bin",
        ".apk" => "apk",
        _ => "bin",
    }
}

/// Get the default target format for the current platform
///
/// # Returns
/// The default build target for the current operating system
pub fn default_target() -> &'static str {
    #[cfg(windows)]
    {
        "exe"
    }
    #[cfg(target_os = "macos")]
    {
        "app"
    }
    #[cfg(target_os = "linux")]
    {
        "bin"
    }
    #[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
    {
        "bin"
    }
}

/// Print help information
pub fn print_help() {
    println!("Nefu - Web-to-Desktop packaging tool");
    println!();
    println!("Usage: nefu <COMMAND> [OPTIONS]");
    println!();
    println!("Commands:");
    println!("  init       Initialize a new Nefu project");
    println!("  start      Start the development server (with hot reload)");
    println!("  build      Build executables (exe/app/bin/.apk)");
    println!("  cpp        Compile C/C++ source files");
    println!("  run        Run scripts");
    println!("  version    Show version information");
    println!("  help       Show help information");
    println!();
    println!("Examples:");
    println!("  nefu init              Create a new project");
    println!("  nefu start             Start the development server");
    println!("  nefu build exe         Build a Windows executable");
    println!("  nefu build app         Build a macOS app");
    println!("  nefu build bin         Build a Linux executable");
    println!("  nefu build .apk        Build an Android APK");
    println!("  nefu build .apk --apk  One-click APK build (auto-downloads dependencies)");
    println!("  nefu cpp main.cpp      Compile C++ source files");
    println!();
    println!("Environment variables:");
    println!("  NEFU_LOG    Log level (trace/debug/info/warn/error)");
    println!();
    println!("For more information visit: https://nefu.dev");
}

/// Parse and validate build arguments
///
/// # Parameters
/// - `args`: build command arguments
///
/// # Returns
/// The validated arguments, or an error if validation fails
pub fn validate_build_args(args: &BuildArgs) -> anyhow::Result<()> {
    // Validate the target platform
    validate_target(&args.target).map_err(|e| anyhow::anyhow!(e))?;

    // If a config file is specified, check that it exists
    if let Some(ref config_path) = args.config {
        if !config_path.exists() {
            return Err(anyhow::anyhow!(
                "specified configuration file does not exist: {}",
                config_path.display()
            ));
        }
    }

    // Warn if both no_compress and no_encrypt are specified
    if args.no_compress && args.no_encrypt {
        log::warn!("both compression and encryption are disabled; the generated file will contain unprotected source code");
    }

    Ok(())
}

/// Format a progress bar for command-line output
///
/// # Parameters
/// - `current`: current progress value
/// - `total`: total progress value
/// - `width`: progress bar width (characters)
///
/// # Returns
/// A formatted progress bar string
pub fn format_progress_bar(current: usize, total: usize, width: usize) -> String {
    if total == 0 {
        return format!("[{}]", "=".repeat(width));
    }

    let filled = (current as f64 / total as f64 * width as f64) as usize;
    let empty = width.saturating_sub(filled);
    let percent = (current as f64 / total as f64 * 100.0) as usize;

    format!(
        "[{}{}] {}%",
        "█".repeat(filled),
        "░".repeat(empty),
        percent
    )
}

/// Print a colored status message
///
/// # Parameters
/// - `status`: status label (e.g. "INFO", "WARN", "ERROR")
/// - `message`: message content
pub fn print_status(status: &str, message: &str) {
    match status {
        "OK" => println!("  ✅ {}", message),
        "INFO" => println!("  ℹ️  {}", message),
        "WARN" => println!("  ⚠️  {}", message),
        "ERROR" => println!("  ❌ {}", message),
        "BUILD" => println!("  🔨 {}", message),
        "PACK" => println!("  📦 {}", message),
        _ => println!("  {} {}", status, message),
    }
}

/// Print a dynamic progress bar with progress (using `\r` to overwrite the current line)
///
/// Suitable for showing real-time progress during time-consuming phases such as compression/encryption.
/// Falls back to line-by-line output when the terminal is not a TTY.
///
/// # Parameters
/// - `current`: current progress
/// - `total`: total progress
/// - `label`: progress description prefix
pub fn print_progress(current: usize, total: usize, label: &str) {
    use std::io::IsTerminal;
    let bar = format_progress_bar(current, total, 24);
    if std::io::stderr().is_terminal() {
        eprint!("\r  📦 {} {}  ", label, bar);
        if current >= total {
            eprintln!();
        }
    } else {
        // Non-TTY: output every 10% to avoid flooding the screen
        if total == 0 || current % (total.max(1) / 10 + 1) == 0 || current >= total {
            println!("  📦 {} {}  {}", label, bar, current);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_target_valid() {
        assert!(validate_target("exe").is_ok());
        assert!(validate_target("app").is_ok());
        assert!(validate_target("bin").is_ok());
        assert!(validate_target(".apk").is_ok());
    }

    #[test]
    fn test_validate_target_invalid() {
        assert!(validate_target("").is_err());
        assert!(validate_target("windows").is_err());
        assert!(validate_target("apk").is_err()); // must include the dot
    }

    #[test]
    fn test_target_extension() {
        assert_eq!(target_extension("exe"), "exe");
        assert_eq!(target_extension("app"), "app");
        assert_eq!(target_extension("bin"), "bin");
        assert_eq!(target_extension(".apk"), "apk");
        assert_eq!(target_extension("unknown"), "bin");
    }

    #[test]
    fn test_format_progress_bar() {
        let bar = format_progress_bar(50, 100, 20);
        assert!(bar.contains("50%"));
        assert!(bar.starts_with('['));
        assert!(bar.ends_with("%"));
    }

    #[test]
    fn test_format_progress_bar_zero_total() {
        let bar = format_progress_bar(0, 0, 20);
        assert!(bar.contains('='));
    }

    #[test]
    fn test_format_progress_bar_complete() {
        let bar = format_progress_bar(100, 100, 20);
        assert!(bar.contains("100%"));
    }

    #[test]
    fn test_cli_parse_start() {
        let cli = Cli::parse_from(["nefu", "start"]);
        match cli.command {
            Commands::Start {
                port,
                host,
                no_reload,
                open,
                watch_extensions,
            } => {
                assert_eq!(port, 3000);
                assert_eq!(host, "127.0.0.1");
                assert!(!no_reload);
                assert!(open);
                assert!(watch_extensions.is_empty());
            }
            _ => panic!("Expected Start command"),
        }
    }

    #[test]
    fn test_cli_parse_start_with_port() {
        let cli = Cli::parse_from(["nefu", "start", "--port", "8080", "--no-reload"]);
        match cli.command {
            Commands::Start { port, no_reload, .. } => {
                assert_eq!(port, 8080);
                assert!(no_reload);
            }
            _ => panic!("Expected Start command"),
        }
    }

    #[test]
    fn test_cli_parse_build() {
        let cli = Cli::parse_from(["nefu", "build", "exe"]);
        match cli.command {
            Commands::Build {
                target,
                config,
                installer,
                output_dir,
                no_compress,
                no_encrypt,
                verbose,
                ..
            } => {
                assert_eq!(target, "exe");
                assert!(config.is_none());
                assert!(!installer);
                assert!(output_dir.is_none());
                assert!(!no_compress);
                assert!(!no_encrypt);
                assert!(!verbose);
            }
            _ => panic!("Expected Build command"),
        }
    }

    #[test]
    fn test_cli_parse_build_with_options() {
        let cli = Cli::parse_from([
            "nefu",
            "build",
            "bin",
            "--output-dir",
            "release",
            "--no-compress",
            "--verbose",
        ]);
        match cli.command {
            Commands::Build {
                target,
                output_dir,
                no_compress,
                verbose,
                ..
            } => {
                assert_eq!(target, "bin");
                assert_eq!(output_dir.unwrap().to_string_lossy(), "release");
                assert!(no_compress);
                assert!(verbose);
            }
            _ => panic!("Expected Build command"),
        }
    }

    #[test]
    fn test_cli_parse_init() {
        let cli = Cli::parse_from(["nefu", "init"]);
        matches!(cli.command, Commands::Init);
    }

    #[test]
    fn test_cli_parse_cpp() {
        let cli = Cli::parse_from(["nefu", "cpp", "main.cpp"]);
        match cli.command {
            Commands::Cpp { source, output, .. } => {
                assert_eq!(source, "main.cpp");
                assert!(output.is_none());
            }
            _ => panic!("Expected Cpp command"),
        }
    }

    #[test]
    fn test_cli_parse_cpp_with_options() {
        let cli = Cli::parse_from(["nefu", "cpp", "main.c", "-o", "app.exe", "-O2", "-std=c17"]);
        match cli.command {
            Commands::Cpp { source, output, options } => {
                assert_eq!(source, "main.c");
                assert_eq!(output.unwrap().to_string_lossy(), "app.exe");
                assert_eq!(options, vec!["-O2", "-std=c17"]);
            }
            _ => panic!("Expected Cpp command"),
        }
    }

    #[test]
    fn test_default_target() {
        let target = default_target();
        assert!(!target.is_empty());
        assert!(["exe", "app", "bin", ".apk"].contains(&target));
    }
}
