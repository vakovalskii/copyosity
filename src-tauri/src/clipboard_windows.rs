//! Windows clipboard signals: the counterpart of the `change_count` / own-write /
//! concealed checks in `clipboard_macos`.
//!
//! `GetClipboardSequenceNumber` plays the role of NSPasteboard `changeCount`: it bumps on
//! every clipboard change and costs nothing to read, so the monitor reads and hashes the
//! payload only when it actually changed (re-hashing a multi-MB bitmap every tick pegged
//! a core on macOS before the same gate was added there).
//!
//! Password managers and other apps that do not want their data recorded put marker
//! formats on the clipboard; we check for those by format id, without reading the payload.

#![cfg(target_os = "windows")]

use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::OnceLock;

use windows_sys::Win32::System::DataExchange::{
    GetClipboardSequenceNumber, IsClipboardFormatAvailable, RegisterClipboardFormatW,
};

static IGNORE_CAPTURE_AT: AtomicI64 = AtomicI64::new(-1);

/// Monotonic clipboard change counter (wraps at u32, which is fine for equality checks).
pub fn change_count() -> i64 {
    // SAFETY: no arguments, reads a global counter.
    i64::from(unsafe { GetClipboardSequenceNumber() })
}

/// Remember the sequence number right after our own write, so the monitor skips it
/// instead of bumping that entry to the top of history.
pub fn mark_own_clipboard_write(change_count: i64) {
    IGNORE_CAPTURE_AT.store(change_count, Ordering::SeqCst);
}

pub fn should_ignore_capture(change_count: i64) -> bool {
    change_count == IGNORE_CAPTURE_AT.load(Ordering::SeqCst)
}

/// Marker formats that mean "do not record this": the Windows clipboard-history contract
/// (`ExcludeClipboardContentFromMonitorProcessing`, `CanIncludeInClipboardHistory`) and the
/// older `Clipboard Viewer Ignore` used by password managers such as KeePass.
const SKIP_HISTORY_FORMATS: &[&str] = &[
    "ExcludeClipboardContentFromMonitorProcessing",
    "Clipboard Viewer Ignore",
];

fn skip_format_ids() -> &'static [u32] {
    static IDS: OnceLock<Vec<u32>> = OnceLock::new();
    IDS.get_or_init(|| {
        SKIP_HISTORY_FORMATS
            .iter()
            .map(|name| {
                let wide: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
                // SAFETY: `wide` is a valid NUL-terminated UTF-16 string for the call.
                unsafe { RegisterClipboardFormatW(wide.as_ptr()) }
            })
            .filter(|id| *id != 0)
            .collect()
    })
}

/// True when the current clipboard content asked not to be recorded. Checks only which
/// formats are present, never reads the data.
pub fn is_concealed() -> bool {
    skip_format_ids()
        .iter()
        // SAFETY: plain format-id query, no clipboard ownership needed.
        .any(|id| unsafe { IsClipboardFormatAvailable(*id) } != 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn own_write_marker_matches_only_its_sequence_number() {
        mark_own_clipboard_write(42);
        assert!(should_ignore_capture(42));
        assert!(!should_ignore_capture(43));
        mark_own_clipboard_write(99);
        assert!(should_ignore_capture(99));
        assert!(!should_ignore_capture(42));
    }
}
