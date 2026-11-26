//! Dummy game mode. Discord detects a game from the running process's
//! executable name and folder, and it also expects the process to own a
//! top-level window (discord-quest-completer's runner shows one too), so we
//! open a small window titled with the game's name. The process exits when
//! the main app exits, so no orphaned "games" are left behind.

pub const FLAG: &str = "--dmg-runner";

pub fn run(args: &[String]) {
    let value = |key: &str| {
        args.windows(2)
            .find(|w| w[0] == key)
            .map(|w| w[1].clone())
    };
    let parent = value("--parent").and_then(|p| p.parse::<u32>().ok());
    let title = value("--title").unwrap_or_else(|| "Game".to_string());
    let minimized = args.iter().any(|a| a == "--minimized");

    #[cfg(windows)]
    {
        if let Some(pid) = parent {
            std::thread::spawn(move || {
                win::wait_for_process(pid);
                std::process::exit(0);
            });
        }
        win::show_window(&title, minimized);
    }

    #[cfg(not(windows))]
    {
        let _ = (parent, title, minimized);
        loop {
            std::thread::sleep(std::time::Duration::from_secs(3600));
        }
    }
}

#[cfg(windows)]
mod win {
    use std::ptr::{null, null_mut};
    use windows_sys::Win32::Foundation::{CloseHandle, HWND, LPARAM, LRESULT, WPARAM};
    use windows_sys::Win32::Graphics::Gdi::COLOR_WINDOW;
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::System::Threading::{
        OpenProcess, WaitForSingleObject, INFINITE, PROCESS_SYNCHRONIZE,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::*;

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    pub fn wait_for_process(pid: u32) {
        unsafe {
            let handle = OpenProcess(PROCESS_SYNCHRONIZE, 0, pid);
            if handle.is_null() {
                return;
            }
            WaitForSingleObject(handle, INFINITE);
            CloseHandle(handle);
        }
    }

    unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
        if msg == WM_DESTROY {
            PostQuitMessage(0);
            return 0;
        }
        DefWindowProcW(hwnd, msg, w, l)
    }

    pub fn show_window(title: &str, minimized: bool) {
        unsafe {
            let hinst = GetModuleHandleW(null());
            let class = wide("DiscordGameVarietyBadgeGame");
            let wc = WNDCLASSW {
                style: 0,
                lpfnWndProc: Some(wndproc),
                cbClsExtra: 0,
                cbWndExtra: 0,
                hInstance: hinst,
                hIcon: LoadIconW(null_mut(), IDI_APPLICATION),
                hCursor: LoadCursorW(null_mut(), IDC_ARROW),
                hbrBackground: (COLOR_WINDOW + 1) as usize as _,
                lpszMenuName: null(),
                lpszClassName: class.as_ptr(),
            };
            RegisterClassW(&wc);

            let title = wide(title);
            let hwnd = CreateWindowExW(
                0,
                class.as_ptr(),
                title.as_ptr(),
                WS_OVERLAPPEDWINDOW,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                360,
                160,
                null_mut(),
                null_mut(),
                hinst,
                null(),
            );
            if hwnd.is_null() {
                return;
            }
            ShowWindow(hwnd, if minimized { SW_SHOWMINNOACTIVE } else { SW_SHOWNOACTIVATE });

            let mut msg: MSG = std::mem::zeroed();
            while GetMessageW(&mut msg, null_mut(), 0, 0) > 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
    }
}
