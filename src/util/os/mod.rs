#[cfg(any(target_os = "windows", target_os = "osx"))]
mod windows_osx;
#[cfg(all(not(target_os = "windows"), not(target_os = "osx")))]
mod non_windows;

#[cfg(any(target_os = "windows", target_os = "osx"))]
pub use self::windows_osx::{file_length, is_device};
#[cfg(all(not(target_os = "windows"), not(target_os = "osx")))]
pub use self::non_windows::{file_length, is_device};
