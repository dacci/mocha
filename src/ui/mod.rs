mod main_frame;

use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::*;

pub use main_frame::MainFrame;

#[allow(unused)]
enum WM {
    Create(*const CREATESTRUCTW),
    Destroy,
    Command(u16, i16, HWND),
    Timer(usize),
    User(u32, WPARAM, LPARAM),
    App(u32, WPARAM, LPARAM),
    Registered(u32, WPARAM, LPARAM),
    Unknown(u32, WPARAM, LPARAM),
}

impl WM {
    fn crack(msg: u32, wp: WPARAM, lp: LPARAM) -> Self {
        match msg {
            WM_CREATE => Self::Create(lp.0 as _),
            WM_DESTROY => Self::Destroy,
            WM_COMMAND => Self::Command((wp.0 >> 16) as _, wp.0 as _, HWND(lp.0 as _)),
            WM_TIMER => Self::Timer(wp.0),
            WM_USER..=0x7FFF => Self::User(msg, wp, lp),
            WM_APP..=0xBFFF => Self::App(msg, wp, lp),
            0xC000..=0xFFFF => Self::Registered(msg, wp, lp),
            _ => Self::Unknown(msg, wp, lp),
        }
    }
}
