/// Nefu build script
/// Embeds the application icon and version info on Windows platforms

fn main() {
    // Only embed resources on Windows platforms
    #[cfg(windows)]
    {
        use std::path::Path;

        let mut res = winres::WindowsResource::new();

        // Set the application icon (if present)
        let icon_path = "resources/app.ico";
        if Path::new(icon_path).exists() {
            res.set_icon(icon_path);
        }

        // Set version info
        res.set("ProductName", "Nefu");
        res.set("FileDescription", "A tool that packages web projects into standalone desktop executables");
        res.set("CompanyName", "Nefu Team");
        res.set("LegalCopyright", "Copyright © 2024 Nefu Team");
        res.set("OriginalFilename", "nefu.exe");

        // Compile resources and embed them into the executable
        if let Err(e) = res.compile() {
            eprintln!("Warning: failed to compile Windows resources: {}", e);
            eprintln!("Hint: make sure the resources/app.ico file exists and is in a valid format");
        }
    }

    // Non-Windows platforms need no special handling
    #[cfg(not(windows))]
    {
        println!("cargo:rerun-if-changed=build.rs");
    }
}
