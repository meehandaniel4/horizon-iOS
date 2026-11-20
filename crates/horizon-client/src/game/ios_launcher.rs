//! Bridge to the embedded iOS launcher framework (`Frameworks/HorizonLauncherKit.framework`).

#[cfg(target_os = "ios")]
pub fn present() -> bool {
    static PRESENTED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if PRESENTED.swap(true, std::sync::atomic::Ordering::Relaxed) {
        return false;
    }

    let framework_path = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("Frameworks/HorizonLauncherKit.framework/HorizonLauncherKit")));

    let Some(path) = framework_path else {
        return false;
    };
    if !path.exists() {
        return false;
    }

    unsafe {
        if let Ok(cpath) = std::ffi::CString::new(path.into_os_string().into_encoded_bytes()) {
            let handle = libc::dlopen(cpath.as_ptr(), libc::RTLD_NOW);
            if handle.is_null() {
                log::warn!("Horizon: could not dlopen HorizonLauncherKit");
                return false;
            }
            let symbol = libc::dlsym(handle, c"horizon_launcher_present".as_ptr());
            if symbol.is_null() {
                log::warn!("Horizon: could not find horizon_launcher_present symbol");
                return false;
            }
            let call = std::mem::transmute::<*mut std::ffi::c_void, extern "C" fn() -> bool>(symbol);
            let result = call();
            log::info!("Horizon: launcher present called, result: {result}");
            result
        } else {
            false
        }
    }
}

#[cfg(not(target_os = "ios"))]
pub fn present() -> bool {
    false
}
