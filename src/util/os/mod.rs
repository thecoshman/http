#[cfg(target_os = "windows")]
mod windows;
#[cfg(not(target_os = "windows"))]
mod non_windows;
#[cfg(any(target_os = "windows", target_os = "macos"))]
mod windows_macos;
#[cfg(not(any(target_os = "windows", target_os = "macos")))]
mod non_windows_non_macos;

#[cfg(target_os = "windows")]
pub use self::windows::*;
#[cfg(not(target_os = "windows"))]
pub use self::non_windows::*;
#[cfg(any(target_os = "windows", target_os = "macos"))]
pub use self::windows_macos::*;
#[cfg(not(any(target_os = "windows", target_os = "macos")))]
pub use self::non_windows_non_macos::*;
