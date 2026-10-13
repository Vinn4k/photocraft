//! `cargo xtask android`: build the Android APK (apps/photocraft-android) with cargo-apk, and
//! optionally install and start it on a phone over adb.
//!
//! Needs the Android SDK (`ANDROID_HOME`, with platform 34 and build-tools), the NDK
//! (`ANDROID_NDK_ROOT`, or the newest under `$ANDROID_HOME/ndk`), a JDK (`keytool`), cargo-apk and
//! the Rust target. `--doctor` lists what is missing and how to get it; docs/development.md ›
//! Android has the one-time setup.
//!
//! Signing: a release build uses the key in `CARGO_APK_RELEASE_KEYSTORE` (+ `_PASSWORD`) when set,
//! else the debug key (`~/.android/debug.keystore`, created on first use), which is enough to
//! install a test build on a phone.

use std::path::{Path, PathBuf};
use std::process::Command;

pub const USAGE: &str = "\
  android [--debug] [--install] [--run] [--target <triple>] [--doctor]
                  build the Android APK (cargo-apk) into target/<profile>/apk/photocraft.apk;
                  --install installs it with adb, --run also starts it; --debug for a dev build;
                  --target defaults to aarch64-linux-android; --doctor checks the toolchain
";

const PACKAGE: &str = "ai.storyteller.photocraft";
const DEFAULT_TARGET: &str = "aarch64-linux-android";

pub fn run(root: &Path, args: &[&str]) -> Result<(), String> {
    let mut release = true;
    let (mut install, mut start, mut doctor) = (false, false, false);
    let mut target = DEFAULT_TARGET.to_string();
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        match *arg {
            "--debug" => release = false,
            "--release" => release = true,
            "--install" => install = true,
            "--run" => (install, start) = (true, true),
            "--doctor" => doctor = true,
            "--target" => target = it.next().ok_or("--target needs a triple (aarch64-linux-android, x86_64-linux-android, …)")?.to_string(),
            other => return Err(format!("android: unknown option `{other}`\n\nusage:\n{USAGE}")),
        }
    }
    let tools = Toolchain::find(&target);
    if doctor {
        return tools.report();
    }
    tools.require()?;
    let sdk = tools.sdk.clone().ok_or("no Android SDK")?;
    let ndk = tools.ndk.clone().ok_or("no Android NDK")?;

    launcher_icons(root)?;
    let mut cmd = Command::new("cargo");
    cmd.current_dir(root).args(["apk", "build", "-p", "photocraft-android", "--lib", "--target", &target]);
    if release {
        cmd.arg("--release");
        if std::env::var_os("CARGO_APK_RELEASE_KEYSTORE").is_none() {
            let key = debug_keystore()?;
            eprintln!("android: signing with the debug key {} (set CARGO_APK_RELEASE_KEYSTORE for a release key)", key.display());
            cmd.env("CARGO_APK_RELEASE_KEYSTORE", &key).env("CARGO_APK_RELEASE_KEYSTORE_PASSWORD", "android");
        }
    }
    cmd.env("ANDROID_HOME", &sdk).env_remove("ANDROID_SDK_ROOT").env("ANDROID_NDK_ROOT", &ndk).env("ANDROID_NDK_HOME", &ndk);
    // robius-file-picker's build script compiles its Java helper with the SDK's d8: the newest
    // build-tools read the newest JDKs' class files (34's d8 can't read JDK 21's).
    if std::env::var_os("ANDROID_BUILD_TOOLS_VERSION").is_none()
        && let Some(version) = newest_dir(&sdk.join("build-tools")).and_then(|d| d.file_name().map(|n| n.to_string_lossy().into_owned()))
    {
        cmd.env("ANDROID_BUILD_TOOLS_VERSION", version);
    }
    eprintln!("android: {}", describe(&cmd));
    let status = cmd.status().map_err(|e| format!("cargo apk: {e}"))?;
    if !status.success() {
        return Err(format!("cargo apk build failed ({status})"));
    }
    let apk = target_dir(root).join(if release { "release" } else { "debug" }).join("apk").join("photocraft.apk");
    if !apk.is_file() {
        return Err(format!("the build finished but {} is missing", apk.display()));
    }
    let size = std::fs::metadata(&apk).map(|m| m.len()).unwrap_or(0);
    println!("APK: {} ({:.1} MB)", apk.display(), size as f64 / 1_048_576.0);
    if install {
        let adb = tools.adb.clone().ok_or("adb not found: install the SDK's platform-tools")?;
        run_tool(Command::new(&adb).args(["install", "-r"]).arg(&apk), "adb install")?;
        if start {
            run_tool(Command::new(&adb).args(["shell", "am", "start", "-n", &format!("{PACKAGE}/android.app.NativeActivity")]), "adb shell am start")?;
            println!("started; logs: {} logcat -s photocraft RustStdoutStderr", adb.display());
        }
    } else {
        println!("install on a phone: adb install -r {} (or copy the file to the phone and open it)", apk.display());
    }
    Ok(())
}

/// Where the Android tools are, or `None` for each one missing.
struct Toolchain {
    target: String,
    sdk: Option<PathBuf>,
    ndk: Option<PathBuf>,
    platform: bool,
    build_tools: bool,
    keytool: bool,
    cargo_apk: bool,
    rust_target: bool,
    adb: Option<PathBuf>,
}

impl Toolchain {
    fn find(target: &str) -> Self {
        let env_dir = |names: &[&str]| names.iter().filter_map(std::env::var_os).map(PathBuf::from).find(|p| p.is_dir());
        let home = std::env::var_os("HOME").map(PathBuf::from);
        let sdk = env_dir(&["ANDROID_HOME", "ANDROID_SDK_ROOT"])
            .or_else(|| [home.as_ref().map(|h| h.join("Android/Sdk")), Some(PathBuf::from("/opt/android-sdk"))].into_iter().flatten().find(|p| p.is_dir()));
        let ndk = env_dir(&["ANDROID_NDK_ROOT", "ANDROID_NDK_HOME"]).or_else(|| sdk.as_ref().and_then(|s| newest_dir(&s.join("ndk"))));
        let has_subdir = |dir: &str| sdk.as_ref().is_some_and(|s| std::fs::read_dir(s.join(dir)).is_ok_and(|mut d| d.next().is_some()));
        let adb = sdk
            .as_ref()
            .map(|s| s.join("platform-tools").join("adb"))
            .filter(|p| p.is_file())
            .or_else(|| succeeds("adb", &["version"]).then(|| PathBuf::from("adb")));
        let rust_target = Command::new("rustup")
            .args(["target", "list", "--installed"])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).lines().any(|l| l.trim() == target))
            // Without rustup (a distribution's Rust), let cargo report a missing target itself.
            .unwrap_or(true);
        Toolchain {
            target: target.to_string(),
            platform: has_subdir("platforms"),
            build_tools: has_subdir("build-tools"),
            sdk,
            ndk,
            keytool: succeeds("keytool", &["-help"]),
            cargo_apk: succeeds("cargo", &["apk", "version"]) || succeeds("cargo", &["apk", "--help"]),
            rust_target,
            adb,
        }
    }

    /// Each requirement with how to get it when missing.
    fn checks(&self) -> Vec<(&'static str, bool, String)> {
        let sdkmanager = "$ANDROID_HOME/cmdline-tools/latest/bin/sdkmanager";
        vec![
            ("Android SDK", self.sdk.is_some(), "install the command-line tools (developer.android.com/studio#command-tools) and set ANDROID_HOME".into()),
            ("SDK platform", self.platform, format!("{sdkmanager} \"platforms;android-34\"")),
            ("SDK build-tools", self.build_tools, format!("{sdkmanager} \"build-tools;35.0.0\"")),
            ("Android NDK", self.ndk.is_some(), format!("{sdkmanager} \"ndk;27.2.12479018\" (or set ANDROID_NDK_ROOT)")),
            ("JDK keytool", self.keytool, "install a JDK (17 or later), e.g. apt install openjdk-17-jdk-headless".into()),
            ("cargo-apk", self.cargo_apk, "cargo install cargo-apk --locked".into()),
            ("Rust target", self.rust_target, format!("rustup target add {}", self.target)),
        ]
    }

    fn report(&self) -> Result<(), String> {
        let mut missing = 0;
        for (name, ok, fix) in self.checks() {
            if ok {
                println!("ok       {name}");
            } else {
                missing += 1;
                println!("missing  {name}: {fix}");
            }
        }
        let adb = self.adb.as_ref().map_or_else(|| "not found (only needed for --install/--run)".to_string(), |p| p.display().to_string());
        println!("adb      {adb}");
        if let Some(s) = &self.sdk {
            println!("SDK      {}", s.display());
        }
        if let Some(n) = &self.ndk {
            println!("NDK      {}", n.display());
        }
        if missing == 0 { Ok(()) } else { Err(format!("{missing} requirement(s) missing")) }
    }

    fn require(&self) -> Result<(), String> {
        let missing: Vec<String> = self.checks().into_iter().filter(|(_, ok, _)| !ok).map(|(name, _, fix)| format!("  {name}: {fix}")).collect();
        if missing.is_empty() { Ok(()) } else { Err(format!("the Android toolchain is incomplete:\n{}\n(cargo xtask android --doctor)", missing.join("\n"))) }
    }
}

/// The launcher icon at each density, from the app icon (assets/app-icon), into
/// apps/photocraft-android/res (gitignored; cargo-apk's `resources`).
fn launcher_icons(root: &Path) -> Result<(), String> {
    let icons = root.join("assets/app-icon/hicolor");
    let res = root.join("apps/photocraft-android/res");
    for (density, size) in [("mdpi", 48), ("xhdpi", 128), ("xxxhdpi", 256)] {
        let from = icons.join(format!("{size}x{size}/apps/{PACKAGE}.png"));
        let dir = res.join(format!("mipmap-{density}"));
        std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        std::fs::copy(&from, dir.join("ic_launcher.png")).map_err(|e| format!("{}: {e}", from.display()))?;
    }
    Ok(())
}

/// The debug signing key every Android tool shares, created when missing.
fn debug_keystore() -> Result<PathBuf, String> {
    let home = std::env::var_os("HOME").map(PathBuf::from).ok_or("HOME is not set")?;
    let dir = home.join(".android");
    let key = dir.join("debug.keystore");
    if key.is_file() {
        return Ok(key);
    }
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let mut cmd = Command::new("keytool");
    cmd.args(["-genkeypair", "-v", "-keystore"])
        .arg(&key)
        .args(["-storepass", "android", "-keypass", "android", "-alias", "androiddebugkey", "-keyalg", "RSA", "-keysize", "2048", "-validity", "10000"])
        .args(["-dname", "CN=Android Debug,O=Android,C=US"]);
    run_tool(&mut cmd, "keytool")?;
    Ok(key)
}

fn newest_dir(dir: &Path) -> Option<PathBuf> {
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(dir).ok()?.filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.is_dir()).collect();
    // NDK folders are versions ("27.2.12479018"): compare them numerically.
    let key = |p: &PathBuf| -> Vec<u64> { p.file_name().map(|n| n.to_string_lossy().split('.').map(|s| s.parse().unwrap_or(0)).collect()).unwrap_or_default() };
    dirs.sort_by_key(key);
    dirs.pop()
}

fn target_dir(root: &Path) -> PathBuf {
    std::env::var_os("CARGO_TARGET_DIR").map(PathBuf::from).map(|p| if p.is_absolute() { p } else { root.join(p) }).unwrap_or_else(|| root.join("target"))
}

fn succeeds(program: &str, args: &[&str]) -> bool {
    Command::new(program).args(args).output().is_ok_and(|o| o.status.success())
}

fn run_tool(cmd: &mut Command, what: &str) -> Result<(), String> {
    let status = cmd.status().map_err(|e| format!("{what}: {e}"))?;
    if status.success() { Ok(()) } else { Err(format!("{what} failed ({status})")) }
}

fn describe(cmd: &Command) -> String {
    std::iter::once(cmd.get_program()).chain(cmd.get_args()).map(|a| a.to_string_lossy().into_owned()).collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn newest_ndk_is_the_highest_version() {
        let dir = std::env::temp_dir().join(format!("xtask-ndk-{}", std::process::id()));
        for v in ["25.2.9519653", "27.2.12479018", "26.1.10909125"] {
            std::fs::create_dir_all(dir.join(v)).unwrap();
        }
        assert_eq!(newest_dir(&dir).unwrap().file_name().unwrap(), "27.2.12479018");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn unknown_options_are_errors() {
        let e = run(Path::new("."), &["--fast"]).unwrap_err();
        assert!(e.contains("unknown option `--fast`"), "{e}");
        assert!(run(Path::new("."), &["--target"]).unwrap_err().contains("needs a triple"));
    }
}
