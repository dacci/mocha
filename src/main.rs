#![allow(non_snake_case)]
#![windows_subsystem = "windows"]

use std::mem::size_of;
use std::ptr::null;
use windows::core::*;
use windows::Win32::Foundation::*;
use windows::Win32::System::LibraryLoader::*;
use windows::Win32::System::Power::*;
use windows::Win32::System::SystemInformation::*;
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::Shell::*;
use windows::Win32::UI::WindowsAndMessaging::*;

static mut WM_TASKBARCREATED: u32 = 0;
static mut AWAKE: bool = false;
static mut PROHIBIT_SS: bool = false;

fn encode(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}

fn encode_a<const N: usize>(s: &str) -> [u16; N] {
    let mut v = encode(s);
    v.resize(N - 1, 0);
    v.push(0);
    v.try_into().unwrap()
}

fn main() {
    unsafe { WM_TASKBARCREATED = RegisterWindowMessageW("TaskbarCreated") };

    let class_name = encode("Mocha");
    let class_name = PCWSTR(class_name.as_ptr());

    let wcex = WNDCLASSEXW {
        cbSize: size_of::<WNDCLASSEXW>() as u32,
        lpfnWndProc: Some(wnd_proc),
        hInstance: unsafe { GetModuleHandleW(PCWSTR::default()) },
        lpszClassName: class_name,
        ..WNDCLASSEXW::default()
    };
    let atom = unsafe { RegisterClassExW(&wcex) };
    if atom == 0 {
        return;
    }

    let hWnd = unsafe {
        CreateWindowExW(
            WS_EX_OVERLAPPEDWINDOW,
            class_name,
            class_name,
            WS_OVERLAPPEDWINDOW,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            HWND(0),
            HMENU(0),
            wcex.hInstance,
            null(),
        )
    };
    if hWnd == HWND(0) {
        return;
    }

    let mut msg = MSG::default();
    loop {
        let r = unsafe { GetMessageW(&mut msg, HWND::default(), 0, 0) };
        match r.0 {
            -1 | 0 => break,
            _ => unsafe {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            },
        }
    }
}

unsafe extern "system" fn wnd_proc(
    hWnd: HWND,
    uMsg: u32,
    wParam: WPARAM,
    lParam: LPARAM,
) -> LRESULT {
    match uMsg {
        WM_CREATE => {
            if let Err(_) = add_icon(hWnd) {
                return LRESULT(-1);
            }

            SendMessageW(hWnd, WM_COMMAND, WPARAM(1), LPARAM(0));
            SendMessageW(hWnd, WM_COMMAND, WPARAM(2), LPARAM(0));
        }
        WM_DESTROY => {
            let icon = NOTIFYICONDATAW {
                cbSize: size_of::<NOTIFYICONDATAW>() as u32,
                hWnd,
                ..NOTIFYICONDATAW::default()
            };
            Shell_NotifyIconW(NIM_DELETE, &icon);
            PostQuitMessage(0);
        }
        WM_COMMAND => match wParam.0 {
            0 => {
                DestroyWindow(hWnd);
            }
            1 => {
                AWAKE = if AWAKE {
                    SetThreadExecutionState(ES_CONTINUOUS);
                    false
                } else {
                    SetThreadExecutionState(
                        ES_CONTINUOUS | ES_SYSTEM_REQUIRED | ES_DISPLAY_REQUIRED,
                    );
                    true
                }
            }
            2 => {
                PROHIBIT_SS = if PROHIBIT_SS {
                    KillTimer(hWnd, 0);
                    false
                } else {
                    SetTimer(hWnd, 0, 1000, None);
                    true
                }
            }
            _ => (),
        },
        WM_TIMER => {
            let mut lii = LASTINPUTINFO {
                cbSize: size_of::<LASTINPUTINFO>() as u32,
                dwTime: 0,
            };
            if GetLastInputInfo(&mut lii).as_bool() {
                let now = GetTickCount();
                let idle_time = if lii.dwTime <= now {
                    now - lii.dwTime
                } else {
                    u32::MAX - lii.dwTime + now
                };

                if 60_000 <= idle_time {
                    let inputs = vec![
                        INPUT {
                            r#type: INPUT_KEYBOARD,
                            Anonymous: INPUT_0 {
                                ki: KEYBDINPUT {
                                    wVk: VK_SHIFT,
                                    wScan: 0,
                                    dwFlags: KEYBD_EVENT_FLAGS(0),
                                    time: 0,
                                    dwExtraInfo: 0,
                                },
                            },
                        },
                        INPUT {
                            r#type: INPUT_KEYBOARD,
                            Anonymous: INPUT_0 {
                                ki: KEYBDINPUT {
                                    wVk: VK_SHIFT,
                                    wScan: 0,
                                    dwFlags: KEYEVENTF_KEYUP,
                                    time: 0,
                                    dwExtraInfo: 0,
                                },
                            },
                        },
                    ];
                    SendInput(&inputs, size_of::<INPUT>() as i32);
                }
            }
        }
        WM_APP => match lParam.0 as u32 {
            WM_CONTEXTMENU => {
                let menu = CreatePopupMenu().unwrap();

                AppendMenuW(
                    menu,
                    if AWAKE { MF_CHECKED } else { MF_ENABLED },
                    1,
                    "&Keep awake",
                );
                AppendMenuW(
                    menu,
                    if PROHIBIT_SS { MF_CHECKED } else { MF_ENABLED },
                    2,
                    "&Prohibit screen saver",
                );
                AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR(null()));
                AppendMenuW(menu, MF_ENABLED, 0, "E&xit");

                SetForegroundWindow(hWnd);
                let (x, y) = ((wParam.0 & 0xFFFF) as i32, (wParam.0 >> 16) as i32);
                TrackPopupMenu(menu, TPM_RIGHTBUTTON, x, y, 0, hWnd, null());
                PostMessageW(hWnd, WM_NULL, WPARAM(0), LPARAM(0));

                DestroyMenu(menu);
            }
            _ => (),
        },
        _ if uMsg == WM_TASKBARCREATED => {
            let _ = add_icon(hWnd);
        }
        _ => return DefWindowProcW(hWnd, uMsg, wParam, lParam),
    }

    LRESULT(0)
}

unsafe fn add_icon(hWnd: HWND) -> std::result::Result<(), ()> {
    let icon = NOTIFYICONDATAW {
        cbSize: size_of::<NOTIFYICONDATAW>() as u32,
        hWnd,
        uFlags: NIF_MESSAGE | NIF_ICON | NIF_TIP | NIF_SHOWTIP,
        uCallbackMessage: WM_APP,
        hIcon: LoadIconW(HINSTANCE(0), IDI_APPLICATION).unwrap(),
        szTip: encode_a("Mocha"),
        Anonymous: NOTIFYICONDATAW_0 {
            uVersion: NOTIFYICON_VERSION_4,
        },
        ..NOTIFYICONDATAW::default()
    };
    if Shell_NotifyIconW(NIM_ADD, &icon).as_bool()
        && Shell_NotifyIconW(NIM_SETVERSION, &icon).as_bool()
    {
        Ok(())
    } else {
        Err(())
    }
}
