#[cfg(target_os = "windows")]
mod windows;
#[cfg(not(target_os = "windows"))]
mod non_windows;

#[cfg(target_os = "windows")]
pub use self::windows::is_device;
#[cfg(not(target_os = "windows"))]
pub use self::non_windows::is_device;
