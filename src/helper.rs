use windows::core::{Error, Result, PCWSTR};
use windows::Win32::Foundation::{HINSTANCE, HWND};

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

pub trait CheckHandle: Sized {
    fn ok(self) -> Result<Self>;
}

macro_rules! impl_check_handle {
    ($t:ty) => {
        impl CheckHandle for $t {
            fn ok(self) -> Result<Self> {
                if self.0 != 0 {
                    Ok(self)
                } else {
                    Err(Error::from_win32())
                }
            }
        }
    };
}

impl_check_handle!(HINSTANCE);
impl_check_handle!(HWND);
