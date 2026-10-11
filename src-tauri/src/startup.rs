use std::{io::Write, path::PathBuf};

pub fn report(error: &str) {
    let smoke = std::env::var_os("FILEMELON_SMOKE_DIR");
    let directory = smoke.clone().map(PathBuf::from).or_else(|| {
        std::env::var_os("APPDATA").map(|path| PathBuf::from(path).join("com.krabby.filemelon"))
    }).unwrap_or_else(std::env::temp_dir);
    let path = directory.join("startup.log");
    let _ = std::fs::create_dir_all(&directory);
    if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(&path) {
        let _ = writeln!(file, "{}: {error}", crate::core::model::timestamp());
    }
    eprintln!("Filemelon could not start: {error}");
    #[cfg(windows)]
    if smoke.is_none() {
        #[link(name = "user32")]
        unsafe extern "system" { fn MessageBoxW(window: *mut std::ffi::c_void, text: *const u16, caption: *const u16, kind: u32) -> i32; }
        let message: Vec<u16> = format!("Filemelon could not start.\n\n{error}\n\nDetails: {}\n\nFor the portable version, check that Microsoft Edge WebView2 Runtime is installed.", path.display()).encode_utf16().chain(Some(0)).collect();
        let title: Vec<u16> = "Filemelon — startup error".encode_utf16().chain(Some(0)).collect();
        unsafe { MessageBoxW(std::ptr::null_mut(), message.as_ptr(), title.as_ptr(), 0x10); }
    }
}
