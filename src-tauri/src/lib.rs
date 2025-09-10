#[cfg(mobile)]
mod mobile;
#[cfg(mobile)]
pub use mobile::*;

mod main;
pub use main::get_platform;