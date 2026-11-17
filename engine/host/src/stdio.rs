//! The protocol stream: the process's stdout, kept for JSON Lines alone.
//!
//! A model's own code may print: a solverforge-cli project enables the
//! `console` feature, whose banner and progress lines go to stdout, and a
//! user's `println!` does too. Either would corrupt the event stream. The
//! engine keeps a duplicate of the original stdout for its events and points
//! the process's stdout at stderr, so anything else printed lands there.

use std::io::{self, Write};

/// The writer events go to: the original stdout, with the process's stdout
/// now pointing at stderr. Falls back to plain stdout if the descriptors
/// cannot be rearranged.
pub(crate) fn protocol_stdout() -> Box<dyn Write> {
    take_stdout().unwrap_or_else(|| Box::new(io::stdout()))
}

#[cfg(unix)]
fn take_stdout() -> Option<Box<dyn Write>> {
    use std::os::fd::{FromRawFd, OwnedFd};

    io::stdout().flush().ok()?;
    // SAFETY: `dup` and `dup2` act on this process's own standard
    // descriptors; the duplicate is a fresh descriptor nothing else owns.
    unsafe {
        let protocol = libc::dup(libc::STDOUT_FILENO);
        if protocol < 0 {
            return None;
        }
        let protocol = OwnedFd::from_raw_fd(protocol);
        if libc::dup2(libc::STDERR_FILENO, libc::STDOUT_FILENO) < 0 {
            return None;
        }
        Some(Box::new(io::LineWriter::new(std::fs::File::from(protocol))))
    }
}

#[cfg(windows)]
fn take_stdout() -> Option<Box<dyn Write>> {
    use std::os::windows::io::{FromRawHandle, OwnedHandle};
    use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
    use windows_sys::Win32::System::Console::{
        GetStdHandle, SetStdHandle, STD_ERROR_HANDLE, STD_OUTPUT_HANDLE,
    };

    io::stdout().flush().ok()?;
    // SAFETY: the standard handles belong to this process. The standard
    // library looks the output handle up on every write, so after
    // `SetStdHandle` its stdout writes go to stderr while the original
    // handle is owned by the protocol writer alone.
    unsafe {
        let original = GetStdHandle(STD_OUTPUT_HANDLE);
        let errors = GetStdHandle(STD_ERROR_HANDLE);
        if original.is_null() || original == INVALID_HANDLE_VALUE || errors.is_null() {
            return None;
        }
        if SetStdHandle(STD_OUTPUT_HANDLE, errors) == 0 {
            return None;
        }
        let protocol = OwnedHandle::from_raw_handle(original);
        Some(Box::new(io::LineWriter::new(std::fs::File::from(protocol))))
    }
}

#[cfg(not(any(unix, windows)))]
fn take_stdout() -> Option<Box<dyn Write>> {
    None
}

/// Installs a subscriber that records nothing, before any model code runs.
///
/// A project built with the `console` feature installs a tracing subscriber
/// on its first solve that formats solver events at debug level; with this
/// one in place its installation fails quietly and those events cost nothing.
pub(crate) fn quiet_tracing() {
    let _ = tracing::subscriber::set_global_default(tracing::subscriber::NoSubscriber::default());
}
