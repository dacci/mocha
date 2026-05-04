#![windows_subsystem = "windows"]

mod helper;
mod ui;

use helper::ScopedHandle;
use windows::Win32::Foundation::GetLastError;
use windows::Win32::System::Threading::CreateMutexW;
use windows::Win32::UI::WindowsAndMessaging::{DispatchMessageW, GetMessageW, TranslateMessage};
use windows::core::{Error, Result, w};

fn main() -> Result<()> {
    let _mutex = unsafe {
        let h = CreateMutexW(None, true, w!("github.com/dacci/mocha")).map(ScopedHandle)?;
        GetLastError().ok()?;
        h
    };

    ui::MainFrame::register_class()?;
    let mut frame = ui::MainFrame::new();
    frame.as_mut().create()?;

    let mut msg = Default::default();
    loop {
        let res = unsafe { GetMessageW(&mut msg, None, 0, 0).0 };
        match res {
            0 => break Ok(()),
            -1 => break Err(Error::from_thread()),
            _ => unsafe {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            },
        }
    }
}
