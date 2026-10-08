use std::env;
use std::error::Error;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::generate;

const WORKSPACE_TOOLCHAIN: &str = "1.98.0";
const WINDOWS_EXTENSION_TOOLCHAIN: &str = "nightly-2026-09-20";

const PHP_UNIT_TESTS: &[&str] = &[
    "biome-smoke.php",
    "command-smoke.php",
    "config-smoke.php",
    "log-smoke.php",
    "plugin-smoke.php",
    "process-output-smoke.php",
    "scheduler-smoke.php",
    "tick-smoke.php",
    "world-block-catalog-smoke.php",
    "world-seed-smoke.php",
];

const PHP_NATIVE_WORLD_TESTS: &[&str] = &[
    "world-composition-smoke.php",
    "world-light-smoke.php",
    "world-mutation-smoke.php",
    "world-parity-smoke.php",
    "world-smoke.php",
];

const PHP_NATIVE_TESTS: &[&str] = &[
    "native-exports-smoke.php",
    "extension-smoke.php",
    "native-world-smoke.php",
    "native-biome-source-smoke.php",
    "native-infinite-smoke.php",
    "native-storage-smoke.php",
    "session-runtime-smoke.php",
];

const PHP_INTEGRATION_TESTS: &[&str] = &[
    "world-sync-smoke.php",
    "multi-view-smoke.php",
    "pending-disconnect-smoke.php",
    "backpressure-view-smoke.php",
    "persistent-join-smoke.php",
    "persistent-infinite-join-smoke.php",
    "server-smoke.php",
];

pub fn setup(root: &Path) -> Result<(), Box<dyn Error>> {
    composer(root, ["install", "--no-interaction", "--prefer-dist"])
}

pub fn build(root: &Path) -> Result<(), Box<dyn Error>> {
    ensure_autoload(root)?;
    native_build(root)
}

pub fn check(root: &Path) -> Result<(), Box<dyn Error>> {
    composer(
        root,
        [
            "validate",
            "--strict",
            "--no-check-publish",
            "composer.json",
        ],
    )?;
    lint_php(root)?;
    generate::run(root, true)?;
    workspace_check(root)?;
    native_check(root)?;
    vendor_php(root, "phpstan", ["analyse", "--no-progress"])?;
    vendor_php(
        root,
        "php-cs-fixer",
        ["fix", "--dry-run", "--diff", "--using-cache=no"],
    )
}

pub fn test(root: &Path) -> Result<(), Box<dyn Error>> {
    ensure_autoload(root)?;
    cargo(
        root,
        root,
        WORKSPACE_TOOLCHAIN,
        ["test", "--locked", "--workspace", "--lib", "--tests"],
    )?;
    native_build(root)?;
    vendor_php(root, "phpunit", ["--configuration", "phpunit.xml.dist"])?;
    test_php(root)
}

pub fn verify(root: &Path) -> Result<(), Box<dyn Error>> {
    check(root)?;
    test(root)
}

pub fn native_build(root: &Path) -> Result<(), Box<dyn Error>> {
    let extension = root.join("native/extension");
    cargo(
        root,
        &extension,
        native_toolchain(),
        ["build", "--locked", "--release"],
    )
}

pub fn native_check(root: &Path) -> Result<(), Box<dyn Error>> {
    let extension = root.join("native/extension");
    cargo(root, &extension, native_toolchain(), ["fmt", "--check"])?;
    cargo(root, &extension, native_toolchain(), ["check", "--locked"])?;
    cargo(
        root,
        &extension,
        native_toolchain(),
        ["clippy", "--locked", "--", "-D", "warnings"],
    )
}

pub fn test_php(root: &Path) -> Result<(), Box<dyn Error>> {
    ensure_autoload(root)?;
    cargo(
        root,
        root,
        WORKSPACE_TOOLCHAIN,
        [
            "build",
            "--locked",
            "-p",
            "cobblestone-client-bootstrap",
            "--bin",
            "world-sync-client",
        ],
    )?;
    php(root, root.join("tests/php/zts-probe.php"), &[], None)?;

    for test in PHP_UNIT_TESTS {
        php(root, root.join("tests/php/unit").join(test), &[], None)?;
    }
    for test in PHP_NATIVE_WORLD_TESTS {
        php_with_extension(root, root.join("tests/php/unit").join(test), &[])?;
    }
    for test in PHP_NATIVE_TESTS {
        php_with_extension(root, root.join("tests/php/native").join(test), &[])?;
    }
    for test in PHP_INTEGRATION_TESTS {
        let script = root.join("tests/php/integration").join(test);
        php_with_extension(root, &script, &[])?;
        if *test == "persistent-infinite-join-smoke.php" {
            php_with_extension(root, &script, &[OsString::from("--wide-initial")])?;
        }
        if *test == "world-sync-smoke.php" {
            for argument in [
                "--transition-only",
                "--radius-cycle",
                "--wide-initial",
                "--stream-torture",
            ] {
                php_with_extension(root, &script, &[OsString::from(argument)])?;
            }
        }
    }
    Ok(())
}

pub fn serve(root: &Path, arguments: Vec<OsString>) -> Result<(), Box<dyn Error>> {
    ensure_autoload(root)?;
    native_build(root)?;
    php(
        root,
        root.join("src/Cobblestone.php"),
        &arguments,
        Some(extension_path(root)),
    )
}

pub fn modules(root: &Path) -> Result<(), Box<dyn Error>> {
    println!("php: src (ardosia/cobblestone)");
    let mut crates = Vec::new();
    for entry in fs::read_dir(root.join("native"))? {
        let entry = entry?;
        if entry.file_type()?.is_dir() && entry.path().join("Cargo.toml").is_file() {
            crates.push(entry.file_name());
        }
    }
    crates.sort();
    for krate in crates {
        println!("rust: {}", krate.to_string_lossy());
    }
    Ok(())
}

fn workspace_check(root: &Path) -> Result<(), Box<dyn Error>> {
    cargo(
        root,
        root,
        WORKSPACE_TOOLCHAIN,
        ["fmt", "--all", "--", "--check"],
    )?;
    cargo(
        root,
        root,
        WORKSPACE_TOOLCHAIN,
        ["check", "--locked", "--workspace", "--all-targets"],
    )?;
    cargo(
        root,
        root,
        WORKSPACE_TOOLCHAIN,
        [
            "clippy",
            "--locked",
            "--workspace",
            "--all-targets",
            "--",
            "-D",
            "warnings",
        ],
    )
}

fn ensure_autoload(root: &Path) -> Result<(), Box<dyn Error>> {
    if !root.join("vendor/autoload.php").is_file() {
        return Err("Composer packages are not installed; run composer setup".into());
    }
    composer(
        root,
        [
            "dump-autoload",
            "--no-interaction",
            "--classmap-authoritative",
        ],
    )
}

fn lint_php(root: &Path) -> Result<(), Box<dyn Error>> {
    let mut files = vec![root.join(".php-cs-fixer.dist.php")];
    for directory in ["app", "src", "tests/php"] {
        collect_php(&root.join(directory), &mut files)?;
    }
    files.sort();
    files.dedup();

    for file in files {
        run(
            root,
            root,
            php_binary(),
            [OsString::from("-l"), file.into_os_string()],
        )?;
    }
    Ok(())
}

fn collect_php(directory: &Path, files: &mut Vec<PathBuf>) -> Result<(), Box<dyn Error>> {
    if !directory.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            collect_php(&path, files)?;
        } else if path.extension() == Some(OsStr::new("php")) {
            files.push(path);
        }
    }
    Ok(())
}

fn php_with_extension(
    root: &Path,
    script: impl AsRef<Path>,
    arguments: &[OsString],
) -> Result<(), Box<dyn Error>> {
    php(root, script, arguments, Some(extension_path(root)))
}

fn php(
    root: &Path,
    script: impl AsRef<Path>,
    arguments: &[OsString],
    extension: Option<PathBuf>,
) -> Result<(), Box<dyn Error>> {
    let mut args = Vec::new();
    if let Some(extension) = extension {
        if !extension.is_file() {
            return Err("native extension is missing; run composer native:build".into());
        }
        args.push(OsString::from("-n"));
        args.push(OsString::from("-d"));
        args.push(OsString::from(format!("extension={}", extension.display())));
    }
    args.push(script.as_ref().as_os_str().to_owned());
    args.extend_from_slice(arguments);
    run(root, root, php_binary(), args)
}

fn composer<I, S>(root: &Path, arguments: I) -> Result<(), Box<dyn Error>>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    if let Some(binary) = env::var_os("COMPOSER_BINARY").filter(|path| Path::new(path).is_file()) {
        let mut args = vec![binary];
        args.extend(arguments.into_iter().map(|arg| arg.as_ref().to_owned()));
        return run(root, root, php_binary(), args);
    }
    if cfg!(windows) {
        // Composer is commonly exposed by setup-php as a .bat/.cmd shim. Windows CreateProcess
        // does not resolve those scripts directly, so route the shim through cmd.exe.
        let mut args = vec![OsString::from("/C"), OsString::from("composer")];
        args.extend(arguments.into_iter().map(|arg| arg.as_ref().to_owned()));
        return run(root, root, OsString::from("cmd.exe"), args);
    }
    run(root, root, OsString::from("composer"), arguments)
}

fn cargo<I, S>(root: &Path, cwd: &Path, toolchain: &str, arguments: I) -> Result<(), Box<dyn Error>>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let mut args = vec![OsString::from(format!("+{toolchain}"))];
    args.extend(arguments.into_iter().map(|arg| arg.as_ref().to_owned()));
    run(root, cwd, OsString::from("cargo"), args)
}

fn run<I, S>(
    root: &Path,
    cwd: &Path,
    program: impl AsRef<OsStr>,
    arguments: I,
) -> Result<(), Box<dyn Error>>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let mut command = Command::new(program.as_ref());
    command.current_dir(cwd).args(arguments);
    println!("+ {}", display_command(&command));
    let status = command.status()?;
    if !status.success() {
        return Err(format!(
            "command failed with {} in {}: {}",
            status,
            cwd.strip_prefix(root).unwrap_or(cwd).display(),
            display_command(&command),
        )
        .into());
    }
    Ok(())
}

fn display_command(command: &Command) -> String {
    let mut parts = vec![command.get_program().to_string_lossy().into_owned()];
    parts.extend(
        command
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned()),
    );
    parts.join(" ")
}

fn native_toolchain() -> &'static str {
    if cfg!(windows) {
        WINDOWS_EXTENSION_TOOLCHAIN
    } else {
        WORKSPACE_TOOLCHAIN
    }
}

fn extension_path(root: &Path) -> PathBuf {
    let name = if cfg!(windows) {
        "cobblestone_core_php.dll"
    } else if cfg!(target_os = "macos") {
        "libcobblestone_core_php.dylib"
    } else {
        "libcobblestone_core_php.so"
    };
    root.join("native/extension/target/release").join(name)
}

fn vendor_php<I, S>(root: &Path, name: &str, arguments: I) -> Result<(), Box<dyn Error>>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let mut args = vec![root.join("vendor/bin").join(name).into_os_string()];
    args.extend(arguments.into_iter().map(|arg| arg.as_ref().to_owned()));
    run(root, root, php_binary(), args)
}

fn php_binary() -> OsString {
    env::var_os("PHP_BINARY").unwrap_or_else(|| OsString::from("php"))
}
