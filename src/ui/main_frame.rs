use super::WM;
use crate::helper::*;
use std::mem::size_of;
use std::pin::Pin;
use std::sync::OnceLock;
use windows::Win32::Foundation::*;
use windows::Win32::System::LibraryLoader::*;
use windows::Win32::System::Power::*;
use windows::Win32::System::SystemInformation::*;
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::Shell::*;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::*;

static TASKBAR_CREATED: OnceLock<u32> = OnceLock::new();

pub struct MainFrame {
    hwnd: HWND,
    awake: bool,
    prohibit_ss: bool,
}

impl MainFrame {
    const CLASS_NAME: PCWSTR = w!("Mocha");

    pub fn register_class() -> Result<u16> {
        let instance = unsafe { GetModuleHandleW(None) }?;
        let class = WNDCLASSEXW {
            cbSize: size_of::<WNDCLASSEXW>() as _,
            lpfnWndProc: Some(Self::wnd_proc),
            cbWndExtra: size_of::<usize>() as _,
            hInstance: instance.into(),
            lpszClassName: Self::CLASS_NAME,
            ..Default::default()
        };
        match unsafe { RegisterClassExW(&class) } {
            0 => Err(Error::from_thread()),
            atom => Ok(atom),
        }
    }

    pub fn new() -> Pin<Box<Self>> {
        Box::pin(MainFrame {
            hwnd: Default::default(),
            awake: false,
            prohibit_ss: false,
        })
    }

    pub fn create(self: Pin<&mut Self>) -> Result<HWND> {
        let instance = unsafe { GetModuleHandleW(None) }?;
        unsafe {
            CreateWindowExW(
                WS_EX_OVERLAPPEDWINDOW,
                Self::CLASS_NAME,
                Self::CLASS_NAME,
                WS_OVERLAPPEDWINDOW,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                None,
                None,
                Some(instance.into()),
                Some(self.get_mut() as *mut _ as _),
            )
        }
    }

    extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
        let self_ = unsafe {
            if msg == WM_NCCREATE {
                let cs = (lp.0 as *const CREATESTRUCTW).as_ref_unchecked();
                SetWindowLongPtrW(hwnd, WINDOW_LONG_PTR_INDEX(0), cs.lpCreateParams as _);

                let self_ = cs.lpCreateParams as *mut Self;
                (*self_).hwnd = hwnd;
                self_
            } else {
                GetWindowLongPtrW(hwnd, WINDOW_LONG_PTR_INDEX(0)) as *mut Self
            }
        };

        match unsafe { self_.as_mut() } {
            Some(self_) => self_.handle(msg, wp, lp),
            None => unsafe { DefWindowProcW(hwnd, msg, wp, lp) },
        }
    }

    fn handle(&mut self, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
        let &taskbar_created =
            TASKBAR_CREATED.get_or_init(|| unsafe { RegisterWindowMessageW(w!("TaskbarCreated")) });

        match WM::crack(msg, wp, lp) {
            WM::Create(cs) => {
                if let Err(e) = self.handle_create(cs) {
                    eprintln!("{e}");
                    return LRESULT(-1);
                }
            }
            WM::Destroy => self.handle_destroy(),
            WM::Command(code, id, hwnd) => self.handle_command(code, id, hwnd),
            WM::Timer(id) => self.handle_timer(id),
            WM::App(msg, wp, lp) => self.handle_app(msg, wp, lp),
            WM::Registered(msg, ..) if msg == taskbar_created => self.handle_taskbar_created(),
            _ => return unsafe { DefWindowProcW(self.hwnd, msg, wp, lp) },
        }

        LRESULT(0)
    }

    fn handle_create(&mut self, _: *const CREATESTRUCTW) -> Result<()> {
        self.add_icon()?;

        unsafe { SendMessageW(self.hwnd, WM_COMMAND, Some(WPARAM(1)), None) };
        unsafe { SendMessageW(self.hwnd, WM_COMMAND, Some(WPARAM(2)), None) };

        Ok(())
    }

    fn handle_destroy(&mut self) {
        let icon = NOTIFYICONDATAW {
            cbSize: size_of::<NOTIFYICONDATAW>() as _,
            hWnd: self.hwnd,
            ..Default::default()
        };
        let _ = unsafe { Shell_NotifyIconW(NIM_DELETE, &icon) };
        unsafe { PostQuitMessage(0) };
    }

    fn handle_command(&mut self, _: u16, id: i16, _: HWND) {
        match id {
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
                    unsafe { SetTimer(Some(self.hwnd), 0, 1000, None) };
                } else {
                    let _ = unsafe { KillTimer(Some(self.hwnd), 0) };
                }
            }
            _ => {}
        }
    }

    fn handle_timer(&mut self, _: usize) {
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

    fn handle_app(&mut self, _: u32, wp: WPARAM, lp: LPARAM) {
        if lp.0 == WM_CONTEXTMENU as _ {
            let (x, y) = ((wp.0 & 0xFFFF) as i32, (wp.0 >> 16) as i32);
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

                let _ = SetForegroundWindow(self.hwnd);
                let _ = TrackPopupMenu(menu, TPM_RIGHTBUTTON, x, y, None, self.hwnd, None);
                let _ = PostMessageW(Some(self.hwnd), WM_NULL, WPARAM(0), LPARAM(0));

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
            ..Default::default()
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
