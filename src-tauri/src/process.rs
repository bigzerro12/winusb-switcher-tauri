//! Cross-platform process helpers.
//!
//! The `NoWindow` trait adds `.no_window()` to `std::process::Command`.
//! On Windows this sets the `CREATE_NO_WINDOW` creation flag so that no
//! console window ever flickers on screen when spawning child processes from
//! a GUI application.  On other platforms the method is a no-op.

use std::process::Command;

pub trait NoWindow {
    fn no_window(&mut self) -> &mut Self;
}

impl NoWindow for Command {
    fn no_window(&mut self) -> &mut Self {
        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            // CREATE_NO_WINDOW — prevents any console window from appearing.
            self.creation_flags(0x0800_0000);
        }
        self
    }
}
