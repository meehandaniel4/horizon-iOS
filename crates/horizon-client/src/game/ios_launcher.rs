//! Bridge to the embedded iOS launcher framework (`Frameworks/HorizonLauncherKit.framework`).

pub fn present() -> bool {
    horizon_threads::ios_launcher_present()
}
