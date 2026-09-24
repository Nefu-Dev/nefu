//! C/C++ compilation support module
//!
//! Provides compilation of C/C++ source files using the system-installed compiler (gcc/g++/clang/cl).
//! Invoked via the `nefu cpp main.cpp` command.

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

/// Compile C/C++ source files into an executable
///
/// Automatically detects the source file type (.c uses the C compiler, .cpp uses the C++ compiler),
/// and invokes the system compiler to produce the executable.
///
/// # Parameters
/// - `source`: source file path
/// - `output`: output file path (optional, defaults to the source file name without extension)
/// - `options`: extra compile options (e.g. -O2, -std=c++17)
///
/// # Returns
/// The generated output file path
pub fn compile(source: &Path, output: Option<&Path>, options: &[String]) -> Result<PathBuf> {
    // Verify that the source file exists
    if !source.exists() {
        anyhow::bail!("source file does not exist: {}", source.display());
    }

    // Detect the source file type
    let ext = source
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();

    if ext != "c" && ext != "cpp" {
        anyhow::bail!(
            "unsupported file type '.{}'; only .c and .cpp files are supported",
            ext
        );
    }

    let is_cpp = ext == "cpp";

    // Determine the output path
    let output_path = match output {
        Some(p) => p.to_path_buf(),
        None => {
            let stem = source.file_stem().unwrap_or_default();
            #[cfg(windows)]
            {
                source.with_file_name(format!("{}.exe", stem.to_string_lossy()))
            }
            #[cfg(not(windows))]
            {
                source.with_file_name(stem)
            }
        }
    };

    // Find an available compiler
    let compiler = if is_cpp {
        find_compiler(&["g++", "clang++", "c++"])
    } else {
        find_compiler(&["gcc", "clang", "cc"])
    };

    let compiler = compiler.ok_or_else(|| {
        anyhow::anyhow!(
            "no C/C++ compiler found. Please install one:\n  \
             Windows: MinGW-w64 (https://www.mingw-w64.org) or Visual Studio\n  \
             macOS:   Xcode Command Line Tools (xcode-select --install)\n  \
             Linux:   apt install build-essential / yum groupinstall 'Development Tools'"
        )
    })?;

    log::info!("using compiler: {} (source file: {})", compiler, source.display());

    // Build the compile command
    let mut cmd = std::process::Command::new(&compiler);

    // Add the source file
    cmd.arg(source);

    // Add extra compile options
    for opt in options {
        cmd.arg(opt);
    }

    // Add the output file argument
    cmd.arg("-o").arg(&output_path);

    // Add default options (if no optimization level is specified)
    let has_opt = options.iter().any(|o| o.starts_with("-O"));
    if !has_opt {
        cmd.arg("-O2");
    }

    // Run the compilation
    log::info!("compile command: {} {} -o {}", compiler, source.display(), output_path.display());

    let status = cmd
        .stdin(std::process::Stdio::inherit())
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .status()
        .with_context(|| format!("failed to launch compiler: {}", compiler))?;

    if !status.success() {
        anyhow::bail!(
            "compilation failed (exit code: {:?})",
            status.code()
        );
    }

    // Verify that the output file was generated
    if !output_path.exists() {
        anyhow::bail!(
            "output file not found after compilation: {}",
            output_path.display()
        );
    }

    let file_size = std::fs::metadata(&output_path)
        .map(|m| m.len())
        .unwrap_or(0);

    log::info!("compilation succeeded! Output: {} ({} bytes)", output_path.display(), file_size);

    Ok(output_path)
}

/// Find an available compiler on the system
///
/// Searches in priority order and returns the first compiler found.
///
/// # Parameters
/// - `candidates`: list of compiler names (sorted by priority)
///
/// # Returns
/// The found compiler path, or None if none are found
fn find_compiler(candidates: &[&str]) -> Option<String> {
    for name in candidates {
        if command_exists(name) {
            return Some(name.to_string());
        }
    }
    None
}

/// Check whether a command is available in PATH
///
/// # Parameters
/// - `cmd`: command name
///
/// # Returns
/// true if the command is available
fn command_exists(cmd: &str) -> bool {
    std::process::Command::new(cmd)
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok()
}
