//! Dummy game mode. Discord detects games by the running process's executable
//! name and folder, so the process only has to exist. It exits when the main
//! app exits, so no orphaned "games" are left behind.

pub const FLAG: &str = "--dmg-runner";

pub fn run(args: &[String]) {
    let parent = args
        .windows(2)
        .find(|w| w[0] == "--parent")
        .and_then(|w| w[1].parse::<u32>().ok());

    #[cfg(windows)]
    if let Some(pid) = parent {
        wait_for_process(pid);
        return;
    }

    let _ = parent;
    loop {
        std::thread::sleep(std::time::Duration::from_secs(3600));
    }
}

#[cfg(windows)]
fn wait_for_process(pid: u32) {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{
        OpenProcess, WaitForSingleObject, INFINITE, PROCESS_SYNCHRONIZE,
    };
    unsafe {
        let handle = OpenProcess(PROCESS_SYNCHRONIZE, 0, pid);
        if handle.is_null() {
            return;
        }
        WaitForSingleObject(handle, INFINITE);
        CloseHandle(handle);
    }
}
