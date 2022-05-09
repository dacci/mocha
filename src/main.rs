#![windows_subsystem = "windows"]

mod helper;
mod main_frame;

use crate::main_frame::MainFrame;
use windows::core::{Error, Result};
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, GetMessageW, TranslateMessage, MSG,
};

fn main() -> Result<()> {
    let _frame = MainFrame::new();

    let mut msg = MSG::default();
    loop {
        let r = unsafe { GetMessageW(&mut msg, None, 0, 0) };
        match r.0 {
            0 => break Ok(()),
            -1 => break Err(Error::from_win32()),
            _ => unsafe {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            },
        }
    }
}
