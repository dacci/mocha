use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::core::PCWSTR;

pub struct WideString(pub Vec<u16>);

impl WideString {
    pub fn as_pcwstr(&self) -> PCWSTR {
        PCWSTR(self.0.as_ptr())
    }

    pub fn to_array<const N: usize>(&self) -> [u16; N] {
        let mut v = self.0.clone();
        v.resize(N - 1, 0);
        v.push(0);
        v.try_into().unwrap()
    }
}

pub trait ToWide {
    fn to_wide(&self) -> WideString;
}

impl ToWide for &str {
    fn to_wide(&self) -> WideString {
        WideString(self.encode_utf16().chain(Some(0)).collect())
    }
}

#[repr(transparent)]
pub struct ScopedHandle(pub HANDLE);

impl Drop for ScopedHandle {
    fn drop(&mut self) {
        if !self.0.is_invalid() {
            let _ = unsafe { CloseHandle(self.0) };
        }
    }
}
