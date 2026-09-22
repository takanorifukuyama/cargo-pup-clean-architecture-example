//! Bounded, isolated compiler processes shared by the test harnesses.

use std::error::Error;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

pub type TestResult<T = ()> = Result<T, Box<dyn Error>>;

static TEMP_ID: AtomicU64 = AtomicU64::new(0);

pub struct Project(pub PathBuf);

impl Project {
    pub fn new() -> TestResult<Self> {
        let time = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        let id = TEMP_ID.fetch_add(1, Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("pup-rust-{}-{time}-{id}", std::process::id()));
        fs::create_dir(&path)?;
        Ok(Self(path))
    }
}

impl Drop for Project {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

// File-backed output avoids pipe deadlocks. On Unix, kill the whole process group.
pub fn execute(command: &mut Command, log: &Path) -> TestResult<(Option<i32>, String)> {
    let file = File::create(log)?;
    command
        .stdout(Stdio::from(file.try_clone()?))
        .stderr(Stdio::from(file));
    command
        .env("CARGO_TERM_COLOR", "never")
        .env("NO_COLOR", "1");
    // Fixtures must not inherit cargo-test locks, wrappers or lint suppression flags.
    for name in [
        "CARGO_TARGET_DIR",
        "CARGO_BUILD_TARGET_DIR",
        "CARGO_BUILD_TARGET",
        "RUSTC",
        "RUSTC_WRAPPER",
        "RUSTC_WORKSPACE_WRAPPER",
        "RUSTFLAGS",
        "CARGO_ENCODED_RUSTFLAGS",
        "CARGO_BUILD_RUSTFLAGS",
    ] {
        command.env_remove(name);
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    let mut child = command.spawn()?;
    let started = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if started.elapsed() > Duration::from_secs(180) {
            #[cfg(unix)]
            {
                let _ = Command::new("kill")
                    .args(["-KILL", "--", &format!("-{}", child.id())])
                    .status();
            }
            let _ = child.kill();
            let _ = child.wait();
            return Err(format!("Command timed out: {command:?}; log: {}", log.display()).into());
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    let output = String::from_utf8_lossy(&fs::read(log)?).into_owned();
    writeln!(
        OpenOptions::new().append(true).open(log)?,
        "\nCOMMAND: {command:?}\nEXIT: {:?}",
        status.code()
    )?;
    Ok((status.code(), output))
}
