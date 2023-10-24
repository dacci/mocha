use crate::helper::*;
use std::mem::size_of;
use std::sync::{Once, OnceLock};
use windows::core::*;
use windows::Win32::Foundation::*;
use windows::Win32::System::LibraryLoader::*;
use windows::Win32::System::Power::*;
use windows::Win32::System::SystemInformation::*;
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::Shell::*;
use windows::Win32::UI::WindowsAndMessaging::*;

static WM_TASKBARCREATED: OnceLock<u32> = OnceLock::new();

static REGISTER_WINDOW_CLASS: Once = Once::new();

pub(crate) struct MainFrame {
    hwnd: HWND,
    awake: bool,
    prohibit_ss: bool,
}

impl MainFrame {
    pub(crate) fn new() -> Result<Box<Self>> {
        WM_TASKBARCREATED.get_or_init(|| unsafe { RegisterWindowMessageW(w!("TaskbarCreated")) });

        let instance = unsafe { GetModuleHandleW(None) }?;
        let class_name = "Mocha".to_wide();
        REGISTER_WINDOW_CLASS.call_once(|| {
            let class = WNDCLASSEXW {
                cbSize: size_of::<WNDCLASSEXW>() as _,
                lpfnWndProc: Some(Self::wnd_proc),
                hInstance: instance.into(),
                lpszClassName: class_name.as_pcwstr(),
                ..Default::default()
            };
            assert_ne!(unsafe { RegisterClassExW(&class) }, 0);
        });

        let mut result = Box::new(MainFrame {
            hwnd: HWND(0),
            awake: false,
            prohibit_ss: false,
        });

        unsafe {
            CreateWindowExW(
                WS_EX_OVERLAPPEDWINDOW,
                class_name.as_pcwstr(),
                class_name.as_pcwstr(),
                WS_OVERLAPPEDWINDOW,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                None,
                None,
                instance,
                Some(result.as_mut() as *mut _ as _),
            )
        }
        .ok()?;

        Ok(result)
    }

    #[allow(unused)]
    unsafe extern "system" fn wnd_proc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        let this = if msg == WM_NCCREATE {
            let cs = lparam.0 as *const CREATESTRUCTW;
            let this = (*cs).lpCreateParams as *mut Self;
            (*this).hwnd = hwnd;
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, this as _);
            this
        } else {
            GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Self
        };

        if let Some(this) = this.as_mut() {
            this.handle(msg, wparam, lparam)
        } else {
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
    }

    fn handle(&mut self, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        match msg {
            WM_CREATE => {
                if let Err(e) = self.handle_create() {
                    eprintln!("{e}");
                    return LRESULT(-1);
                }
            }
            WM_DESTROY => self.handle_destroy(),
            WM_COMMAND => self.handle_command(wparam, lparam),
            WM_TIMER => self.handle_timer(),
            WM_APP => self.handle_app(wparam, lparam),
            _ if msg == *WM_TASKBARCREATED.get().unwrap() => self.handle_taskbar_created(),
            _ => return unsafe { DefWindowProcW(self.hwnd, msg, wparam, lparam) },
        }

        LRESULT(0)
    }

    fn handle_create(&mut self) -> Result<()> {
        self.add_icon()?;

        unsafe { SendMessageW(self.hwnd, WM_COMMAND, WPARAM(1), LPARAM(0)) };
        unsafe { SendMessageW(self.hwnd, WM_COMMAND, WPARAM(2), LPARAM(0)) };

        Ok(())
    }

    fn handle_destroy(&mut self) {
        let icon = NOTIFYICONDATAW {
            cbSize: size_of::<NOTIFYICONDATAW>() as _,
            hWnd: self.hwnd,
            ..Default::default()
        };
        unsafe { Shell_NotifyIconW(NIM_DELETE, &icon) };
        unsafe { PostQuitMessage(0) };
    }

    fn handle_command(&mut self, wparam: WPARAM, _: LPARAM) {
        match wparam.0 {
            0 => {
                let _ = unsafe { DestroyWindow(self.hwnd) };
            }
            1 => {
                self.awake = !self.awake;

                let flags = if self.awake {
                    ES_CONTINUOUS | ES_SYSTEM_REQUIRED | ES_DISPLAY_REQUIRED
                } else {
                    ES_CONTINUOUS
                };
                unsafe { SetThreadExecutionState(flags) };
            }
            2 => {
                self.prohibit_ss = !self.prohibit_ss;

                if self.prohibit_ss {
                    unsafe { SetTimer(self.hwnd, 0, 1000, None) };
                } else {
                    let _ = unsafe { KillTimer(self.hwnd, 0) };
                }
            }
            _ => {}
        }
    }

    fn handle_timer(&mut self) {
        let mut lii = LASTINPUTINFO {
            cbSize: size_of::<LASTINPUTINFO>() as _,
            dwTime: 0,
        };
        if unsafe { GetLastInputInfo(&mut lii) }.as_bool() {
            let now = unsafe { GetTickCount() };
            let idle_time = now.wrapping_sub(lii.dwTime);

            if 60_000 <= idle_time {
                let inputs = [
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
                unsafe { SendInput(&inputs, size_of::<INPUT>() as _) };
            }
        }
    }

    fn handle_app(&mut self, wparam: WPARAM, lparam: LPARAM) {
        if lparam.0 == WM_CONTEXTMENU as _ {
            let (x, y) = ((wparam.0 & 0xFFFF) as i32, (wparam.0 >> 16) as i32);
            unsafe {
                let menu = CreatePopupMenu().unwrap();

                let _ = AppendMenuW(
                    menu,
                    if self.awake { MF_CHECKED } else { MF_ENABLED },
                    1,
                    w!("&Keep awake"),
                );
                let _ = AppendMenuW(
                    menu,
                    if self.prohibit_ss {
                        MF_CHECKED
                    } else {
                        MF_ENABLED
                    },
                    2,
                    w!("&Prohibit screen saver"),
                );
                let _ = AppendMenuW(menu, MF_SEPARATOR, 0, None);
                let _ = AppendMenuW(menu, MF_ENABLED, 0, w!("E&xit"));

                SetForegroundWindow(self.hwnd);
                let _ = TrackPopupMenu(menu, TPM_RIGHTBUTTON, x, y, 0, self.hwnd, None);
                let _ = PostMessageW(self.hwnd, WM_NULL, WPARAM(0), LPARAM(0));

                let _ = DestroyMenu(menu);
            }
        }
    }

    fn handle_taskbar_created(&mut self) {
        let _ = self.add_icon();
    }

    fn add_icon(&self) -> Result<()> {
        let tip = "Mocha".to_wide();
        let icon = NOTIFYICONDATAW {
            cbSize: size_of::<NOTIFYICONDATAW>() as _,
            hWnd: self.hwnd,
            uFlags: NIF_MESSAGE | NIF_ICON | NIF_TIP | NIF_SHOWTIP,
            uCallbackMessage: WM_APP,
            hIcon: unsafe { LoadIconW(None, IDI_APPLICATION) }?,
            szTip: tip.to_array(),
            Anonymous: NOTIFYICONDATAW_0 {
                uVersion: NOTIFYICON_VERSION_4,
            },
            ..NOTIFYICONDATAW::default()
        };
        if unsafe { Shell_NotifyIconW(NIM_ADD, &icon) }.as_bool()
            && unsafe { Shell_NotifyIconW(NIM_SETVERSION, &icon) }.as_bool()
        {
            Ok(())
        } else {
            Err(Error::from(E_FAIL))
        }
    }
}
