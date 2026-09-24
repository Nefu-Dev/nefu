use std::path::Path;
use anyhow::{Context, Result};

/// Generate an Android project template for a Nefu project
///
/// This function creates a complete Android project structure in the given output directory,
/// including the user's web resources and a WebView Activity that loads them.
/// The generated project can be built into an APK with Android Studio or Gradle.
///
/// # Arguments
/// - `project_dir`: the original Nefu project directory (contains the HTML/CSS/JS compiled from .nc)
/// - `output_dir`: directory that will hold the generated Android project
/// - `app_name`: the application name
pub fn generate_android_project(project_dir: &Path, output_dir: &Path, app_name: &str, entry_file: &str) -> Result<()> {
    log::info!("generating Android project at: {}", output_dir.display());

    // 1. Create the directory structure
    create_android_dirs(output_dir)?;

    // 2. Copy web resources to the assets directory
    let assets_dir = output_dir.join("app/src/main/assets/web");
    copy_web_resources(project_dir, &assets_dir)?;

    // 3. Write AndroidManifest.xml
    write_manifest(output_dir, app_name)?;

    // 4. Write MainActivity.java (using the configured entry file)
    write_main_activity(output_dir, app_name, entry_file)?;

    // 5. Write NefuBridge.java
    write_nefu_bridge(output_dir)?;

    // 6. Write the project-level build.gradle
    write_project_gradle(output_dir)?;

    // 7. Write the app-level build.gradle
    write_app_gradle(output_dir, app_name)?;

    // 8. Write strings.xml
    write_strings_xml(output_dir, app_name)?;

    // 9. Write the theme styles
    write_themes_xml(output_dir)?;

    // 10. Write the launcher icon placeholder
    write_launcher_icon(output_dir)?;

    // 11. Write the Gradle wrapper properties
    write_gradle_wrapper(output_dir)?;

    // 12. Write settings.gradle
    write_settings_gradle(output_dir)?;

    // 13. Write gradle.properties
    write_gradle_properties(output_dir)?;

    // 14. Write proguard-rules.pro
    write_proguard_rules(output_dir)?;

    // 15. Write the gradlew scripts (Windows + Unix)
    write_gradlew_scripts(output_dir)?;

    // 16. Write local.properties (pointing to the Android SDK)
    write_local_properties(output_dir)?;

    log::info!("Android project generated: {}", output_dir.display());
    Ok(())
}

/// Build an APK automatically with Gradle
///
/// This function will:
/// 1. Ensure the Java JDK is installed
/// 2. Ensure the Android SDK is installed
/// 3. Ensure Gradle is installed
/// 4. Run gradle assembleDebug to build the APK
/// 5. Return the generated APK path
///
/// # Arguments
/// - `project_dir`: the Android project directory (contains build.gradle)
///
/// # Returns
/// The generated APK file path
pub fn build_apk(project_dir: &Path) -> Result<std::path::PathBuf> {
    log::info!("starting automatic APK build...");

    // 1. Ensure the dependencies are installed
    if !crate::utils::check_and_download_dep(crate::utils::DepType::Java) {
        anyhow::bail!("Java JDK is unavailable; cannot build the Android APK");
    }

    if !crate::utils::check_and_download_dep(crate::utils::DepType::AndroidSdk) {
        anyhow::bail!("Android SDK is unavailable; cannot build the Android APK");
    }

    // 2. Get Gradle
    let gradle_bin = crate::utils::ensure_gradle()
        .context("failed to obtain Gradle")?;
    log::info!("using Gradle: {}", gradle_bin.display());

    // 3. Set environment variables
    let java_home = crate::utils::get_java_executable()
        .and_then(|p| p.parent().and_then(|bin| bin.parent()).map(|p| p.to_path_buf()));
    let android_sdk = crate::utils::get_android_sdk_path();

    let mut env_vars: Vec<(String, String)> = Vec::new();
    if let Some(ref jh) = java_home {
        env_vars.push(("JAVA_HOME".to_string(), jh.to_string_lossy().to_string()));
    }
    if let Some(ref sdk) = android_sdk {
        env_vars.push(("ANDROID_HOME".to_string(), sdk.to_string_lossy().to_string()));
        env_vars.push(("ANDROID_SDK_ROOT".to_string(), sdk.to_string_lossy().to_string()));
    }

    // 4. Write/update local.properties
    if let Some(ref sdk) = android_sdk {
        let local_props = format!("sdk.dir={}\n", sdk.to_string_lossy().replace('\\', "/"));
        std::fs::write(project_dir.join("local.properties"), local_props).ok();
    }

    // 5. Run gradle clean assembleDebug
    log::info!("running Gradle build (assembleDebug)...");
    let mut cmd = std::process::Command::new(&gradle_bin);
    cmd.arg("assembleDebug")
        .arg("--no-daemon")
        .current_dir(project_dir)
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit());

    for (k, v) in &env_vars {
        cmd.env(k, v);
    }

    let status = cmd.status().context("Gradle build failed")?;
    if !status.success() {
        anyhow::bail!("Gradle build failed, exit code: {:?}", status.code());
    }

    // 6. Locate the generated APK
    let apk_path = project_dir.join("app/build/outputs/apk/debug/app-debug.apk");
    if !apk_path.exists() {
        // Try other possible paths
        let output_dir = project_dir.join("app/build/outputs/apk");
        if output_dir.exists() {
            for entry in walkdir::WalkDir::new(&output_dir) {
                if let Ok(entry) = entry {
                    if entry.path().extension().map(|e| e == "apk").unwrap_or(false) {
                        return Ok(entry.path().to_path_buf());
                    }
                }
            }
        }
        anyhow::bail!("build finished but no APK file found, expected path: {}", apk_path.display());
    }

    let apk_size = std::fs::metadata(&apk_path).map(|m| m.len()).unwrap_or(0);
    log::info!("APK built successfully: {} ({} bytes)", apk_path.display(), apk_size);

    Ok(apk_path)
}

/// Create the directory structure required by the Android project
fn create_android_dirs(base_dir: &Path) -> Result<()> {
    let dirs = [
        base_dir.join("app/src/main/assets"),
        base_dir.join("app/src/main/java/com/nefu/app"),
        base_dir.join("app/src/main/res/values"),
        base_dir.join("app/src/main/res/mipmap-anydpi-v26"),
        base_dir.join("app/src/main/res/drawable"),
        base_dir.join("gradle/wrapper"),
    ];
    for dir in dirs {
        std::fs::create_dir_all(&dir)?;
    }
    Ok(())
}

/// Copy web resources to the Android assets directory
///
/// Always copies from the project root (not dist/, because dist/ holds the encrypted desktop binaries).
/// Automatically excludes build artifacts, caches, version control, and similar directories.
fn copy_web_resources(project_dir: &Path, assets_dir: &Path) -> Result<()> {
    log::info!("copying web resources to the Android assets directory...");

    if !project_dir.exists() {
        log::warn!("project directory does not exist; assets will be empty");
        return Ok(());
    }

    // Clean up the old assets/web first to avoid re-copying leftover files
    if assets_dir.exists() {
        let _ = std::fs::remove_dir_all(assets_dir);
    }
    std::fs::create_dir_all(assets_dir)?;

    copy_dir_recursively(project_dir, assets_dir)?;

    // Count the copied entries to confirm the copy is not stuck
    let count = std::fs::read_dir(assets_dir)
        .map(|it| it.count())
        .unwrap_or(0);
    log::info!("web resources copied: {} top-level entries -> {}", count, assets_dir.display());

    Ok(())
}

/// Directory names to exclude (not copied to Android assets)
fn is_excluded_dir(name: &str) -> bool {
    matches!(
        name,
        "dist" | "android" | "target" | ".nefu" | ".git" | ".cargo"
            | "node_modules" | ".idea" | ".vscode" | "build" | "deps"
            | "nefu.exe.WebView2" | ".gradle"
    )
}

/// Recursively copy a directory (excluding build artifacts and source config files)
///
/// Note: you must use `filter_entry` to prune before entering a directory.
/// With a `continue` in a for loop, WalkDir would still descend into excluded directories
/// (such as a generated android/ inside the project), causing assets to copy itself in an infinite loop.
fn copy_dir_recursively(src: &Path, dst: &Path) -> Result<()> {
    std::fs::create_dir_all(dst)?;

    let walker = walkdir::WalkDir::new(src)
        .min_depth(1)
        .into_iter()
        .filter_entry(|entry| {
            let path = entry.path();
            if path == src {
                return true;
            }
            // Directories: if they match the exclusion list, skip the whole subtree (do not descend)
            if entry.file_type().is_dir() {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    return !is_excluded_dir(name);
                }
            }
            true
        });

    for entry in walker {
        let entry = entry?;
        let path = entry.path();
        let relative_path = path.strip_prefix(src).unwrap_or(path);
        let target_path = dst.join(relative_path);

        if entry.file_type().is_dir() {
            std::fs::create_dir_all(&target_path)?;
        } else {
            // Skip .nc source files, .toml configs, executables, logs, etc.
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                if matches!(ext, "nc" | "toml" | "exe" | "pdb" | "log" | "bin" | "app" | "apk") {
                    continue;
                }
            }
            // Skip hidden files (e.g. .gitignore) and the nefu executable
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if name.starts_with('.') || name == "nefu" || name == "nefu.exe" {
                    continue;
                }
            }
            if let Some(parent) = target_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::copy(path, &target_path)?;
        }
    }
    Ok(())
}

/// Write AndroidManifest.xml
fn write_manifest(base_dir: &Path, app_name: &str) -> Result<()> {
    let manifest = format!(r#"<?xml version="1.0" encoding="utf-8"?>
<manifest xmlns:android="http://schemas.android.com/apk/res/android"
    package="com.nefu.app">

    <uses-permission android:name="android.permission.INTERNET" />
    <uses-permission android:name="android.permission.ACCESS_NETWORK_STATE" />

    <application
        android:allowBackup="true"
        android:icon="@mipmap/ic_launcher"
        android:label="{app_name}"
        android:roundIcon="@mipmap/ic_launcher_round"
        android:supportsRtl="true"
        android:theme="@style/Theme.NefuApp"
        android:usesCleartextTraffic="true">
        <activity
            android:name=".MainActivity"
            android:exported="true"
            android:configChanges="orientation|screenSize|keyboardHidden"
            android:screenOrientation="portrait">
            <intent-filter>
                <action android:name="android.intent.action.MAIN" />
                <category android:name="android.intent.category.LAUNCHER" />
            </intent-filter>
        </activity>
    </application>
</manifest>
"#);
    std::fs::write(base_dir.join("app/src/main/AndroidManifest.xml"), manifest)?;
    Ok(())
}

/// Write MainActivity.java, using a WebView to load the local assets
fn write_main_activity(base_dir: &Path, _app_name: &str, entry_file: &str) -> Result<()> {
    // Ensure the entry file path is well-formed (strip leading ./ and /)
    let entry = entry_file.trim_start_matches("./").trim_start_matches('/');
    let asset_url = format!("file:///android_asset/web/{}", entry);

    let activity = format!(r#"package com.nefu.app;

import android.annotation.SuppressLint;
import android.os.Bundle;
import android.view.KeyEvent;
import android.webkit.WebChromeClient;
import android.webkit.WebSettings;
import android.webkit.WebView;
import android.webkit.WebViewClient;

import androidx.appcompat.app.AppCompatActivity;

public class MainActivity extends AppCompatActivity {{

    private WebView webView;

    @SuppressLint("SetJavaScriptEnabled")
    @Override
    protected void onCreate(Bundle savedInstanceState) {{
        super.onCreate(savedInstanceState);
        
        webView = new WebView(this);
        setContentView(webView);

        WebSettings settings = webView.getSettings();
        settings.setJavaScriptEnabled(true);
        settings.setDomStorageEnabled(true);
        settings.setLoadWithOverviewMode(true);
        settings.setUseWideViewPort(true);
        settings.setAllowFileAccess(true);
        settings.setAllowContentAccess(true);
        settings.setMediaPlaybackRequiresUserGesture(false);
        settings.setMixedContentMode(WebSettings.MIXED_CONTENT_COMPATIBILITY_MODE);
        
        // Enable the Nefu JS bridge
        webView.addJavascriptInterface(new NefuBridge(this), "NefuAndroid");

        webView.setWebViewClient(new WebViewClient());
        webView.setWebChromeClient(new WebChromeClient());

        // Load the locally packaged web resources
        webView.loadUrl("{asset_url}");
    }}

    @Override
    public boolean onKeyDown(int keyCode, KeyEvent event) {{
        if (keyCode == KeyEvent.KEYCODE_BACK && webView.canGoBack()) {{
            webView.goBack();
            return true;
        }}
        return super.onKeyDown(keyCode, event);
    }}

    @Override
    protected void onDestroy() {{
        if (webView != null) {{
            webView.destroy();
        }}
        super.onDestroy();
    }}
}}
"#, asset_url = asset_url);
    std::fs::write(base_dir.join("app/src/main/java/com/nefu/app/MainActivity.java"), activity)?;
    Ok(())
}

/// Write NefuBridge.java, providing an Android-side JS-Rust bridge replacement
fn write_nefu_bridge(base_dir: &Path) -> Result<()> {
    let bridge = r#"package com.nefu.app;

import android.content.Context;
import android.widget.Toast;
import android.webkit.JavascriptInterface;

/**
 * Provides native bridging for the WebView.
 * This is the Android implementation of Nefu's desktop IPC.
 */
public class NefuBridge {
    private Context context;

    public NefuBridge(Context context) {
        this.context = context;
    }

    @JavascriptInterface
    public void send(String data) {
        // Simplified: currently only handles basic interactions
        // Can be extended later with native dialogs, file access, etc.
    }

    @JavascriptInterface
    public String invoke(String method, String args) {
        switch (method) {
            case "platform":
                return "android";
            case "version":
                return "1.0.0";
            default:
                return null;
        }
    }

    @JavascriptInterface
    public void showToast(String message) {
        Toast.makeText(context, message, Toast.LENGTH_SHORT).show();
    }
}
"#;
    std::fs::write(base_dir.join("app/src/main/java/com/nefu/app/NefuBridge.java"), bridge)?;
    Ok(())
}

/// Write the project-level build.gradle
fn write_project_gradle(base_dir: &Path) -> Result<()> {
    let gradle = r#"// Top-level build file
// Project dependency repositories are managed in settings.gradle (including CN mirrors)
buildscript {
    repositories {
        // The buildscript classpath needs its own repositories declared
        maven { url 'https://maven.aliyun.com/repository/google' }
        maven { url 'https://maven.aliyun.com/repository/central' }
        maven { url 'https://maven.aliyun.com/repository/gradle-plugin' }
        google()
        mavenCentral()
    }
    dependencies {
        classpath 'com.android.tools.build:gradle:8.1.0'
    }
}

task clean(type: Delete) {
    delete rootProject.buildDir
}
"#;
    std::fs::write(base_dir.join("build.gradle"), gradle)?;
    Ok(())
}

/// Write the app-level build.gradle
fn write_app_gradle(base_dir: &Path, _app_name: &str) -> Result<()> {
    let gradle = r#"plugins {
    id 'com.android.application'
}

android {
    namespace 'com.nefu.app'
    compileSdk 34

    defaultConfig {
        applicationId "com.nefu.app"
        minSdk 24
        targetSdk 34
        versionCode 1
        versionName "1.0"
    }

    buildTypes {
        release {
            minifyEnabled true
            proguardFiles getDefaultProguardFile('proguard-android-optimize.txt'), 'proguard-rules.pro'
        }
    }
    
    compileOptions {
        sourceCompatibility JavaVersion.VERSION_1_8
        targetCompatibility JavaVersion.VERSION_1_8
    }
}

dependencies {
    implementation 'androidx.appcompat:appcompat:1.6.1'
    implementation 'com.google.android.material:material:1.11.0'
    implementation 'androidx.constraintlayout:constraintlayout:2.1.4'
}
"#;
    std::fs::write(base_dir.join("app/build.gradle"), gradle)?;
    Ok(())
}

/// Write strings.xml
fn write_strings_xml(base_dir: &Path, app_name: &str) -> Result<()> {
    let strings = format!(r#"<?xml version="1.0" encoding="utf-8"?>
<resources>
    <string name="app_name">{app_name}</string>
</resources>
"#);
    std::fs::write(base_dir.join("app/src/main/res/values/strings.xml"), strings)?;
    Ok(())
}

/// Write themes.xml
fn write_themes_xml(base_dir: &Path) -> Result<()> {
    let themes = r#"<?xml version="1.0" encoding="utf-8"?>
<resources>
    <style name="Theme.NefuApp" parent="Theme.MaterialComponents.DayNight.DarkActionBar">
        <item name="android:windowFullscreen">true</item>
        <item name="android:statusBarColor">#000000</item>
    </style>
</resources>
"#;
    std::fs::write(base_dir.join("app/src/main/res/values/themes.xml"), themes)?;
    Ok(())
}

/// Write the placeholder icon (adaptive icon XML)
fn write_launcher_icon(base_dir: &Path) -> Result<()> {
    let icon_xml = r##"<?xml version="1.0" encoding="utf-8"?>
<adaptive-icon xmlns:android="http://schemas.android.com/apk/res/android">
    <background android:drawable="@drawable/ic_launcher_background" />
    <foreground android:drawable="@drawable/ic_launcher_foreground" />
</adaptive-icon>
"##;
    let round_icon_xml = icon_xml; // simplified: reuse the same icon
    
    let drawable_bg = r##"<?xml version="1.0" encoding="utf-8"?>
<shape xmlns:android="http://schemas.android.com/apk/res/android"
    android:shape="rectangle">
    <solid android:color="#3498db" />
</shape>
"##;
    let drawable_fg = r##"<?xml version="1.0" encoding="utf-8"?>
<vector xmlns:android="http://schemas.android.com/apk/res/android"
    android:width="108dp"
    android:height="108dp"
    android:viewportWidth="108"
    android:viewportHeight="108">
    <path
        android:fillColor="#FFFFFF"
        android:pathData="M54,54m-40,0a40,40 0,1 1,80 0a40,40 0,1 1,-80 0" />
</vector>
"##;
    
    std::fs::write(base_dir.join("app/src/main/res/mipmap-anydpi-v26/ic_launcher.xml"), icon_xml)?;
    std::fs::write(base_dir.join("app/src/main/res/mipmap-anydpi-v26/ic_launcher_round.xml"), round_icon_xml)?;
    std::fs::write(base_dir.join("app/src/main/res/drawable/ic_launcher_background.xml"), drawable_bg)?;
    std::fs::write(base_dir.join("app/src/main/res/drawable/ic_launcher_foreground.xml"), drawable_fg)?;
    
    Ok(())
}

/// Write the Gradle wrapper properties (using a CN mirror for faster downloads)
pub fn write_gradle_wrapper(base_dir: &Path) -> Result<()> {
    let properties = r#"distributionBase=GRADLE_USER_HOME
distributionPath=wrapper/dists
distributionUrl=https\://mirrors.cloud.tencent.com/gradle/gradle-8.4-bin.zip
zipStoreBase=GRADLE_USER_HOME
zipStorePath=wrapper/dists
"#;
    std::fs::write(base_dir.join("gradle/wrapper/gradle-wrapper.properties"), properties)?;
    Ok(())
}

/// Write settings.gradle (with CN Maven mirrors for faster downloads)
fn write_settings_gradle(base_dir: &Path) -> Result<()> {
    let settings = r#"pluginManagement {
    repositories {
        // Prefer CN mirrors
        maven { url 'https://maven.aliyun.com/repository/google' }
        maven { url 'https://maven.aliyun.com/repository/central' }
        maven { url 'https://maven.aliyun.com/repository/gradle-plugin' }
        google()
        mavenCentral()
        gradlePluginPortal()
    }
}
dependencyResolutionManagement {
    repositoriesMode.set(RepositoriesMode.FAIL_ON_PROJECT_REPOS)
    repositories {
        // Prefer CN mirrors
        maven { url 'https://maven.aliyun.com/repository/google' }
        maven { url 'https://maven.aliyun.com/repository/central' }
        google()
        mavenCentral()
    }
}

rootProject.name = "NefuApp"
include ':app'
"#;
    std::fs::write(base_dir.join("settings.gradle"), settings)?;
    Ok(())
}

/// Write gradle.properties
fn write_gradle_properties(base_dir: &Path) -> Result<()> {
    let props = r#"# Project-wide Gradle settings.
org.gradle.jvmargs=-Xmx2048m -Dfile.encoding=UTF-8
org.gradle.parallel=true
org.gradle.caching=true
android.useAndroidX=true
android.nonTransitiveRClass=true
"#;
    std::fs::write(base_dir.join("gradle.properties"), props)?;
    Ok(())
}

/// Write proguard-rules.pro
fn write_proguard_rules(base_dir: &Path) -> Result<()> {
    let rules = r#"# Add project specific ProGuard rules here.
-keepattributes *Annotation*
-keepattributes SourceFile,LineNumberTable
-keep public class * extends android.app.Activity
-keep class com.nefu.app.** { *; }
"#;
    std::fs::write(base_dir.join("app/proguard-rules.pro"), rules)?;
    Ok(())
}

/// Write the gradlew and gradlew.bat scripts
fn write_gradlew_scripts(base_dir: &Path) -> Result<()> {
    // gradlew (Unix)
    let gradlew_unix = r#"#!/bin/sh
# Gradle start up script for UN*X
# Generated by Nefu

# Attempt to set APP_HOME
PRG="$0"
while [ -h "$PRG" ] ; do
    ls=`ls -ld "$PRG"`
    link=`expr "$ls" : '.*-> \(.*\)$'`
    if expr "$link" : '/.*' > /dev/null; then
        PRG="$link"
    else
        PRG=`dirname "$PRG"`"/$link"
    fi
done
SAVED="`pwd`"
cd "`dirname \"$PRG\"`/" >/dev/null
APP_HOME="`pwd -P`"
cd "$SAVED" >/dev/null

APP_NAME="Gradle"
APP_BASE_NAME=`basename "$0"`

DEFAULT_JVM_OPTS='"-Xmx64m" "-Xms64m"'

MAX_FD="maximum"

warn () {
    echo "$*"
} >&2

die () {
    echo
    echo "$*"
    echo
    exit 1
} >&2

# OS specific support
cygwin=false
msys=false
darwin=false
nonstop=false
case "`uname`" in
  CYGWIN* )
    cygwin=true
    ;;
  Darwin* )
    darwin=true
    ;;
  MINGW* )
    msys=true
    ;;
  NONSTOP* )
    nonstop=true
    ;;
esac

CLASSPATH=$APP_HOME/gradle/wrapper/gradle-wrapper.jar

# Determine the Java command to use
if [ -n "$JAVA_HOME" ] ; then
    if [ -x "$JAVA_HOME/jre/sh/java" ] ; then
        JAVACMD="$JAVA_HOME/jre/sh/java"
    else
        JAVACMD="$JAVA_HOME/bin/java"
    fi
    if [ ! -x "$JAVACMD" ] ; then
        die "ERROR: JAVA_HOME is set to an invalid directory: $JAVA_HOME"
    fi
else
    JAVACMD="java"
    which java >/dev/null 2>&1 || die "ERROR: JAVA_HOME is not set and no 'java' command could be found in your PATH."
fi

# Increase the maximum file descriptors if we can
if [ "$cygwin" = "false" -a "$darwin" = "false" -a "$nonstop" = "false" ] ; then
    MAX_FD_LIMIT=`ulimit -H -n`
    if [ $? -eq 0 ] ; then
        if [ "$MAX_FD" = "maximum" -o "$MAX_FD" = "max" ] ; then
            MAX_FD="$MAX_FD_LIMIT"
        fi
        ulimit -n $MAX_FD
        if [ $? -ne 0 ] ; then
            warn "Could not set maximum file descriptor limit: $MAX_FD"
        fi
    else
        warn "Could not query maximum file descriptor limit: $MAX_FD_LIMIT"
    fi
fi

# For Darwin, add options to specify how the application appears in the dock
if $darwin; then
    GRADLE_OPTS="$GRADLE_OPTS \"-Xdock:name=$APP_NAME\" \"-Xdock:icon=$APP_HOME/media/gradle.icns\""
fi

# For Cygwin or MSYS, switch paths to Windows format before running java
if [ "$cygwin" = "true" -o "$msys" = "true" ] ; then
    APP_HOME=`cygpath --path --mixed "$APP_HOME"`
    CLASSPATH=`cygpath --path --mixed "$CLASSPATH"`
    JAVACMD=`cygpath --unix "$JAVACMD"`
fi

# Collect all arguments for the java command
eval set -- $DEFAULT_JVM_OPTS $JAVA_OPTS $GRADLE_OPTS "\"-Dorg.gradle.appname=$APP_BASE_NAME\"" -classpath "\"$CLASSPATH\"" org.gradle.wrapper.GradleWrapperMain "$APP_ARGS"

exec "$JAVACMD" "$@"
"#;
    std::fs::write(base_dir.join("gradlew"), gradlew_unix)?;

    // gradlew.bat (Windows)
    let gradlew_win = r#"@rem Gradle start up script for Windows
@rem Generated by Nefu
@if "%DEBUG%" == "" @echo off
@rem Set local scope for the variables with windows NT shell
if "%OS%"=="Windows_NT" setlocal

set DIRNAME=%~dp0
if "%DIRNAME%" == "" set DIRNAME=.
set APP_BASE_NAME=%~n0
set APP_HOME=%DIRNAME%

@rem Resolve any "." and ".." in APP_HOME to make it shorter.
for %%i in ("%APP_HOME%") do set APP_HOME=%%~fi

@rem Add default JVM options here.
set DEFAULT_JVM_OPTS="-Xmx64m" "-Xms64m"

@rem Find java.exe
if defined JAVA_HOME goto findJavaFromJavaHome

set JAVA_EXE=java.exe
%JAVA_EXE% -version >NUL 2>&1
if "%ERRORLEVEL%" == "0" goto execute

echo.
echo ERROR: JAVA_HOME is not set and no 'java' command could be found in your PATH.
echo.
echo Please set the JAVA_HOME variable in your environment to match the
echo location of your Java installation.
goto fail

:findJavaFromJavaHome
set JAVA_HOME=%JAVA_HOME:"=%
set JAVA_EXE=%JAVA_HOME%/bin/java.exe

if exist "%JAVA_EXE%" goto execute

echo.
echo ERROR: JAVA_HOME is set to an invalid directory: %JAVA_HOME%
echo.
echo Please set the JAVA_HOME variable in your environment to match the
echo location of your Java installation.
goto fail

:execute
@rem Setup the command line
set CLASSPATH=%APP_HOME%\gradle\wrapper\gradle-wrapper.jar

@rem Execute Gradle
"%JAVA_EXE%" %DEFAULT_JVM_OPTS% %JAVA_OPTS% %GRADLE_OPTS% "-Dorg.gradle.appname=%APP_BASE_NAME%" -classpath "%CLASSPATH%" org.gradle.wrapper.GradleWrapperMain %*

:end
@rem End local scope for the variables with windows NT shell
if "%ERRORLEVEL%"=="0" goto mainEnd

:fail
rem Set variable GRADLE_EXIT_CONSOLE if you need the _script_ return code instead of
rem the _cmd.exe /c_ return code!
if  not "" == "%GRADLE_EXIT_CONSOLE%" exit 1
exit /b 1

:mainEnd
if "%OS%"=="Windows_NT" endlocal

:omega
"#;
    std::fs::write(base_dir.join("gradlew.bat"), gradlew_win)?;

    // Set the gradlew executable permission on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(meta) = std::fs::metadata(base_dir.join("gradlew")) {
            let mut perms = meta.permissions();
            perms.set_mode(0o755);
            let _ = std::fs::set_permissions(base_dir.join("gradlew"), perms);
        }
    }

    Ok(())
}

/// Write local.properties (pointing to the Android SDK)
fn write_local_properties(base_dir: &Path) -> Result<()> {
    // Try to get the SDK path from an environment variable or the local cache
    let sdk_path = std::env::var("ANDROID_HOME")
        .or_else(|_| std::env::var("ANDROID_SDK_ROOT"))
        .unwrap_or_else(|_| {
            let cached = crate::utils::nefu_deps_dir().join("android-sdk");
            cached.to_string_lossy().to_string()
        });

    let sdk_path_normalized = sdk_path.replace('\\', "/");
    let content = format!(
        "## This file is automatically generated by Nefu.\n# Do not modify this file.\n#\nsdk.dir={}\n",
        sdk_path_normalized
    );
    std::fs::write(base_dir.join("local.properties"), content)?;
    Ok(())
}
