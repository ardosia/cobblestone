mod generate;
mod project;

use std::env;
use std::error::Error;
use std::ffi::OsString;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn Error>> {
    let root = portable_root(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .canonicalize()?,
    );
    let mut args = env::args_os().skip(1);
    let Some(command) = args.next() else {
        usage();
    };

    match command.to_string_lossy().as_ref() {
        "generate" => {
            let check = match args.next() {
                None => false,
                Some(argument) if argument == "--check" => true,
                Some(_) => usage(),
            };
            if args.next().is_some() {
                usage();
            }
            generate::run(&root, check)?;
        }
        "setup" => no_args(args, || project::setup(&root))?,
        "build" => no_args(args, || project::build(&root))?,
        "check" => no_args(args, || project::check(&root))?,
        "test" => no_args(args, || project::test(&root))?,
        "verify" => no_args(args, || project::verify(&root))?,
        "native-build" => no_args(args, || project::native_build(&root))?,
        "native-check" => no_args(args, || project::native_check(&root))?,
        "test-php" => no_args(args, || project::test_php(&root))?,
        "modules" => no_args(args, || project::modules(&root))?,
        "serve" => project::serve(&root, args.collect())?,
        _ => usage(),
    }

    Ok(())
}

fn portable_root(path: PathBuf) -> PathBuf {
    #[cfg(windows)]
    {
        let rendered = path.to_string_lossy();
        if let Some(rest) = rendered.strip_prefix(r"\\?\UNC\") {
            return PathBuf::from(format!(r"\\{rest}"));
        }
        if let Some(rest) = rendered.strip_prefix(r"\\?\") {
            return PathBuf::from(rest);
        }
    }
    path
}

fn no_args<F>(mut args: impl Iterator<Item = OsString>, run: F) -> Result<(), Box<dyn Error>>
where
    F: FnOnce() -> Result<(), Box<dyn Error>>,
{
    if args.next().is_some() {
        usage();
    }
    run()
}

fn usage() -> ! {
    eprintln!(
        "usage: cargo xtask {{generate [--check]|setup|build|check|test|verify|native-build|native-check|test-php|modules|serve [args...]}}"
    );
    std::process::exit(64);
}
