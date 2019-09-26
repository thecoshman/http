#[cfg(any(target_os = "windows", target_os = "macos"))]
mod windows_macos;
#[cfg(not(any(target_os = "windows", target_os = "macos")))]
mod non_windows_non_macos;

#[cfg(any(target_os = "windows", target_os = "macos"))]
pub use self::windows_macos::*;
#[cfg(not(any(target_os = "windows", target_os = "macos")))]
pub use self::non_windows_non_macos::*;
