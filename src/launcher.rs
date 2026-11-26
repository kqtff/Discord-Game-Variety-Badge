//! Creating and starting the dummy game executables.
//!
//! Same approach as discord-quest-completer: put an executable with the game's
//! name in `games/<app id>/<path>/` next to the app and run it. Discord sees
//! the process and shows the game as being played.

use crate::games::Game;
use std::fs;
use std::io;
use std::path::PathBuf;
use std::process::{Child, Command};

pub fn base_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."))
}

/// Makes `games/<id>/<exe path>`. It is a hard link to this app's own .exe,
/// so 100 games take no extra disk space. Falls back to a copy where hard
/// links aren't supported (e.g. FAT32/exFAT USB drives).
fn prepare(game: &Game) -> io::Result<PathBuf> {
    let me = std::env::current_exe()?;
    let target = base_dir().join("games").join(&game.id).join(&game.exe_path);
    if let Some(dir) = target.parent() {
        fs::create_dir_all(dir)?;
    }
    if target.exists() {
        // Left over from an older version of the app: replace it.
        let same = fs::metadata(&target)?.len() == fs::metadata(&me)?.len();
        if same {
            return Ok(target);
        }
        let _ = fs::remove_file(&target);
    }
    if fs::hard_link(&me, &target).is_err() {
        fs::copy(&me, &target)?;
    }
    Ok(target)
}

pub fn start(game: &Game) -> io::Result<Child> {
    let exe = prepare(game)?;
    let mut cmd = Command::new(&exe);
    cmd.arg(crate::runner::FLAG)
        .arg("--parent")
        .arg(std::process::id().to_string());
    if let Some(dir) = exe.parent() {
        cmd.current_dir(dir);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let child = cmd.spawn()?;
    job::assign(&child);
    Ok(child)
}

/// Removes the `games` folder. Only call when nothing is running.
pub fn clean() -> io::Result<()> {
    let dir = base_dir().join("games");
    if dir.exists() {
        fs::remove_dir_all(dir)?;
    }
    Ok(())
}

/// On Windows every dummy game goes into a job object that kills them all
/// when the app closes, even if it crashes.
#[cfg(windows)]
mod job {
    use std::os::windows::io::AsRawHandle;
    use std::sync::OnceLock;
    use windows_sys::Win32::System::JobObjects::*;

    struct Job(usize);
    static JOB: OnceLock<Option<Job>> = OnceLock::new();

    fn handle() -> Option<usize> {
        JOB.get_or_init(|| unsafe {
            let h = CreateJobObjectW(std::ptr::null(), std::ptr::null());
            if h.is_null() {
                return None;
            }
            let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
            info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            SetInformationJobObject(
                h,
                JobObjectExtendedLimitInformation,
                &info as *const _ as *const _,
                std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            );
            Some(Job(h as usize))
        })
        .as_ref()
        .map(|j| j.0)
    }

    pub fn assign(child: &std::process::Child) {
        if let Some(job) = handle() {
            unsafe {
                AssignProcessToJobObject(job as _, child.as_raw_handle() as _);
            }
        }
    }
}

#[cfg(not(windows))]
mod job {
    pub fn assign(_child: &std::process::Child) {}
}
