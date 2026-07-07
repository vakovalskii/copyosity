//! Accessibility tree walk, focus restore, trust checks.

#[cfg(target_os = "macos")]
use std::sync::atomic::Ordering;

#[cfg(target_os = "macos")]
use objc2_foundation::NSString;

#[cfg(target_os = "macos")]
use super::{paste_log, restore_paste_target, FocusRef, PASTE_TARGET_FOCUS, PASTE_TARGET_PID};

/// Bundle IDs where `AXPaste` is unreliable; use synthetic Cmd+V instead.
#[cfg(any(target_os = "macos", test))]
pub(crate) const KEYBOARD_PASTE_BUNDLE_IDS: &[&str] = &[
    "com.apple.MobileSMS",
    "com.apple.iChat",
    // Chromium browsers — AXPaste often reports success without inserting into web fields.
    "com.brave.Browser",
    "com.google.Chrome",
    "org.chromium.Chromium",
    "com.microsoft.edgemac",
    "com.operasoftware.Opera",
    "com.operasoftware.OperaGX",
    "com.vivaldi.Vivaldi",
    "company.thebrowser.Browser",
    "com.kagi.kagimacOS",
    "com.google.Chrome.canary",
    "com.brave.Browser.beta",
];

#[cfg(any(target_os = "macos", test))]
pub(crate) fn bundle_prefers_keyboard_paste(bundle_id: &str) -> bool {
    KEYBOARD_PASTE_BUNDLE_IDS.contains(&bundle_id)
}

/// Apps where `AXPaste` is unreliable; use synthetic Cmd+V instead.
#[cfg(target_os = "macos")]
pub(crate) fn prefers_keyboard_paste(pid: i32) -> bool {
    crate::macos_app::app_identity_for_pid(pid)
        .is_some_and(|identity| bundle_prefers_keyboard_paste(&identity.bundle_id))
}

#[cfg(target_os = "macos")]
pub(crate) fn try_ax_paste_for_pid(pid: i32) -> bool {
    !prefers_keyboard_paste(pid) && try_ax_paste()
}

#[cfg(target_os = "macos")]
pub(crate) fn try_ax_paste() -> bool {
    const K_AX_ERROR_SUCCESS: i32 = 0;

    let Some(FocusRef(element)) = *PASTE_TARGET_FOCUS.lock().unwrap() else {
        return false;
    };

    unsafe {
        let action = NSString::from_str("AXPaste");
        ax_ui_element_perform_action(element, objc2::rc::Retained::as_ptr(&action).cast())
            == K_AX_ERROR_SUCCESS
    }
}

#[cfg(target_os = "macos")]
pub(crate) fn store_focused_ui_element(element: Option<*mut std::ffi::c_void>) {
    let mut guard = PASTE_TARGET_FOCUS.lock().unwrap();
    if let Some(FocusRef(old)) = guard.take() {
        unsafe { release_ax_element(old) };
    }
    if let Some(element) = element.filter(|ptr| !ptr.is_null()) {
        // `AXUIElementCopyAttributeValue` returns a +1 reference; we take ownership here.
        *guard = Some(FocusRef(element));
    }
}

#[cfg(target_os = "macos")]
pub(crate) fn refresh_paste_focus_if_needed() {
    if PASTE_TARGET_FOCUS.lock().unwrap().is_some() {
        return;
    }

    let pid = PASTE_TARGET_PID.load(Ordering::SeqCst);
    if pid <= 0 {
        return;
    }

    if let Some(element) = capture_focus_for_pid(pid) {
        paste_log("refresh focus=ok");
        store_focused_ui_element(Some(element));
        restore_paste_target();
    } else {
        paste_log("refresh focus=failed");
    }
}

#[cfg(target_os = "macos")]
pub(crate) fn capture_focus_for_pid(pid: i32) -> Option<*mut std::ffi::c_void> {
    copy_focused_ui_element_for_pid(pid)
        .or_else(copy_focused_ui_element_system)
        .or_else(|| find_editable_element_for_pid(pid))
}

#[cfg(target_os = "macos")]
fn copy_focused_ui_element_for_pid(pid: i32) -> Option<*mut std::ffi::c_void> {
    unsafe {
        let app = ax_create_application(pid)?;
        let element = copy_attribute_as_element(app, "AXFocusedUIElement")?;
        release_ax_element(app);
        Some(element)
    }
}

#[cfg(target_os = "macos")]
fn copy_focused_ui_element_system() -> Option<*mut std::ffi::c_void> {
    unsafe {
        let system = ax_create_system_wide()?;
        let element = copy_attribute_as_element(system, "AXFocusedUIElement")?;
        release_ax_element(system);
        Some(element)
    }
}

#[cfg(target_os = "macos")]
fn find_editable_element_for_pid(pid: i32) -> Option<*mut std::ffi::c_void> {
    unsafe {
        let app = ax_create_application(pid)?;

        if let Some(window) = copy_attribute_as_element(app, "AXFocusedWindow") {
            let found = find_editable_element(window, 0);
            release_ax_element(window);
            if found.is_some() {
                release_ax_element(app);
                return found;
            }
        }

        if let Some(windows) = copy_attribute_as_element_array(app, "AXWindows") {
            for (idx, window) in windows.iter().copied().enumerate() {
                if let Some(found) = find_editable_element(window, 0) {
                    for (other_idx, other) in windows.iter().copied().enumerate() {
                        if other_idx != idx {
                            release_ax_element(other);
                        }
                    }
                    release_ax_element(window);
                    release_ax_element(app);
                    return Some(found);
                }
                release_ax_element(window);
            }
        }

        release_ax_element(app);
        None
    }
}

#[cfg(target_os = "macos")]
fn editable_role_priority(role: &str) -> Option<u8> {
    match role {
        "AXTextArea" => Some(0),
        "AXTextField" => Some(1),
        "AXSearchField" => Some(2),
        "AXComboBox" => Some(3),
        "AXWebArea" => Some(4),
        // Last resort — message lists in Messages are scroll areas, not paste targets.
        "AXScrollArea" => Some(5),
        _ => None,
    }
}

#[cfg(target_os = "macos")]
unsafe fn find_editable_element(
    element: *mut std::ffi::c_void,
    depth: u8,
) -> Option<*mut std::ffi::c_void> {
    let mut best: Option<(u8, *mut std::ffi::c_void)> = None;
    find_best_editable_element(element, depth, &mut best);
    best.map(|(_, element)| element)
}

#[cfg(target_os = "macos")]
unsafe fn find_best_editable_element(
    element: *mut std::ffi::c_void,
    depth: u8,
    best: &mut Option<(u8, *mut std::ffi::c_void)>,
) {
    if depth > 12 {
        return;
    }

    if let Some(role) = copy_attribute_as_string(element, "AXRole") {
        if let Some(priority) = editable_role_priority(&role) {
            let replace = best
                .as_ref()
                .is_none_or(|(best_priority, _)| priority < *best_priority);
            if replace {
                if let Some((_, old)) = best.take() {
                    release_ax_element(old);
                }
                cf_retain(element);
                *best = Some((priority, element));
            }
        }
    }

    let Some(children) = copy_attribute_as_element_array(element, "AXChildren") else {
        return;
    };
    for child in children {
        find_best_editable_element(child, depth + 1, best);
        release_ax_element(child);
    }
}

#[cfg(target_os = "macos")]
unsafe fn copy_attribute_as_element(
    element: *mut std::ffi::c_void,
    attribute: &str,
) -> Option<*mut std::ffi::c_void> {
    const K_AX_ERROR_SUCCESS: i32 = 0;

    let attr = NSString::from_str(attribute);
    let mut value: *mut std::ffi::c_void = std::ptr::null_mut();
    let err = ax_copy_attribute_value(
        element,
        objc2::rc::Retained::as_ptr(&attr).cast(),
        &mut value,
    );
    if err == K_AX_ERROR_SUCCESS && !value.is_null() {
        Some(value)
    } else {
        None
    }
}

#[cfg(target_os = "macos")]
unsafe fn copy_attribute_as_string(
    element: *mut std::ffi::c_void,
    attribute: &str,
) -> Option<String> {
    let attr = NSString::from_str(attribute);
    let mut value: *mut std::ffi::c_void = std::ptr::null_mut();
    let err = ax_copy_attribute_value(
        element,
        objc2::rc::Retained::as_ptr(&attr).cast(),
        &mut value,
    );
    if err != 0 || value.is_null() {
        return None;
    }

    let s = cf_string_to_rust(value);
    cf_release(value);
    s
}

#[cfg(target_os = "macos")]
unsafe fn copy_attribute_as_element_array(
    element: *mut std::ffi::c_void,
    attribute: &str,
) -> Option<Vec<*mut std::ffi::c_void>> {
    let attr = NSString::from_str(attribute);
    let mut value: *mut std::ffi::c_void = std::ptr::null_mut();
    let err = ax_copy_attribute_value(
        element,
        objc2::rc::Retained::as_ptr(&attr).cast(),
        &mut value,
    );
    if err != 0 || value.is_null() {
        return None;
    }

    let items = cf_array_to_elements(value);
    cf_release(value);
    if items.is_empty() {
        None
    } else {
        Some(items)
    }
}

#[cfg(target_os = "macos")]
unsafe fn cf_string_to_rust(value: *mut std::ffi::c_void) -> Option<String> {
    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        fn CFStringGetLength(str: *mut std::ffi::c_void) -> isize;
        fn CFStringGetCStringPtr(str: *mut std::ffi::c_void, encoding: u32) -> *const i8;
        fn CFStringGetCString(
            str: *mut std::ffi::c_void,
            buffer: *mut i8,
            buffer_size: isize,
            encoding: u32,
        ) -> bool;
    }

    const K_CF_STRING_ENCODING_UTF8: u32 = 0x0800_0100;

    let direct = CFStringGetCStringPtr(value, K_CF_STRING_ENCODING_UTF8);
    if !direct.is_null() {
        return Some(
            std::ffi::CStr::from_ptr(direct)
                .to_string_lossy()
                .into_owned(),
        );
    }

    let len = CFStringGetLength(value);
    if len <= 0 {
        return None;
    }
    let mut buf = vec![0i8; (len as usize) * 4 + 1];
    if CFStringGetCString(
        value,
        buf.as_mut_ptr(),
        buf.len() as isize,
        K_CF_STRING_ENCODING_UTF8,
    ) {
        Some(
            std::ffi::CStr::from_ptr(buf.as_ptr())
                .to_string_lossy()
                .into_owned(),
        )
    } else {
        None
    }
}

#[cfg(target_os = "macos")]
unsafe fn cf_array_to_elements(value: *mut std::ffi::c_void) -> Vec<*mut std::ffi::c_void> {
    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        fn CFGetTypeID(cf: *mut std::ffi::c_void) -> usize;
        fn CFArrayGetTypeID() -> usize;
        fn CFArrayGetCount(array: *mut std::ffi::c_void) -> isize;
        fn CFArrayGetValueAtIndex(
            array: *mut std::ffi::c_void,
            idx: isize,
        ) -> *const std::ffi::c_void;
    }

    if CFGetTypeID(value) != CFArrayGetTypeID() {
        return Vec::new();
    }

    let count = CFArrayGetCount(value);
    let mut out = Vec::with_capacity(count as usize);
    for idx in 0..count {
        let ptr = CFArrayGetValueAtIndex(value, idx) as *mut std::ffi::c_void;
        if !ptr.is_null() {
            cf_retain(ptr);
            out.push(ptr);
        }
    }
    out
}

#[cfg(target_os = "macos")]
pub(crate) fn restore_focused_ui_element(pid: i32, element: *mut std::ffi::c_void) {
    const K_AX_ERROR_SUCCESS: i32 = 0;

    unsafe {
        // Prefer system-wide focus restore (works better for Electron webviews).
        if let Some(system) = ax_create_system_wide() {
            let attr = NSString::from_str("AXFocusedUIElement");
            let err =
                ax_set_attribute_value(system, objc2::rc::Retained::as_ptr(&attr).cast(), element);
            release_ax_element(system);
            if err == K_AX_ERROR_SUCCESS {
                return;
            }
        }

        let Some(app) = ax_create_application(pid) else {
            return;
        };
        let attr = NSString::from_str("AXFocusedUIElement");
        let _err = ax_set_attribute_value(app, objc2::rc::Retained::as_ptr(&attr).cast(), element);
        release_ax_element(app);
    }
}

#[cfg(target_os = "macos")]
unsafe fn ax_create_system_wide() -> Option<*mut std::ffi::c_void> {
    let system = ax_ui_element_create_system_wide();
    if system.is_null() {
        None
    } else {
        Some(system)
    }
}

#[cfg(target_os = "macos")]
unsafe fn ax_create_application(pid: i32) -> Option<*mut std::ffi::c_void> {
    let app = ax_ui_element_create_application(pid);
    if app.is_null() {
        None
    } else {
        Some(app)
    }
}

#[cfg(target_os = "macos")]
unsafe fn release_ax_element(element: *mut std::ffi::c_void) {
    cf_release(element);
}

#[cfg(target_os = "macos")]
unsafe fn ax_copy_attribute_value(
    element: *mut std::ffi::c_void,
    attribute: *const std::ffi::c_void,
    value: *mut *mut std::ffi::c_void,
) -> i32 {
    ax_ui_element_copy_attribute_value(element, attribute, value)
}

#[cfg(target_os = "macos")]
unsafe fn ax_set_attribute_value(
    element: *mut std::ffi::c_void,
    attribute: *const std::ffi::c_void,
    value: *mut std::ffi::c_void,
) -> i32 {
    ax_ui_element_set_attribute_value(element, attribute, value)
}

#[cfg(target_os = "macos")]
unsafe fn ax_ui_element_create_system_wide() -> *mut std::ffi::c_void {
    #[link(name = "ApplicationServices", kind = "framework")]
    extern "C" {
        fn AXUIElementCreateSystemWide() -> *mut std::ffi::c_void;
    }
    AXUIElementCreateSystemWide()
}

#[cfg(target_os = "macos")]
unsafe fn ax_ui_element_create_application(pid: i32) -> *mut std::ffi::c_void {
    #[link(name = "ApplicationServices", kind = "framework")]
    extern "C" {
        fn AXUIElementCreateApplication(pid: i32) -> *mut std::ffi::c_void;
    }
    AXUIElementCreateApplication(pid)
}

#[cfg(target_os = "macos")]
unsafe fn ax_ui_element_copy_attribute_value(
    element: *mut std::ffi::c_void,
    attribute: *const std::ffi::c_void,
    value: *mut *mut std::ffi::c_void,
) -> i32 {
    #[link(name = "ApplicationServices", kind = "framework")]
    extern "C" {
        fn AXUIElementCopyAttributeValue(
            element: *mut std::ffi::c_void,
            attribute: *const std::ffi::c_void,
            value: *mut *mut std::ffi::c_void,
        ) -> i32;
    }
    AXUIElementCopyAttributeValue(element, attribute, value)
}

#[cfg(target_os = "macos")]
unsafe fn ax_ui_element_set_attribute_value(
    element: *mut std::ffi::c_void,
    attribute: *const std::ffi::c_void,
    value: *mut std::ffi::c_void,
) -> i32 {
    #[link(name = "ApplicationServices", kind = "framework")]
    extern "C" {
        fn AXUIElementSetAttributeValue(
            element: *mut std::ffi::c_void,
            attribute: *const std::ffi::c_void,
            value: *mut std::ffi::c_void,
        ) -> i32;
    }
    AXUIElementSetAttributeValue(element, attribute, value)
}

#[cfg(target_os = "macos")]
unsafe fn ax_ui_element_perform_action(
    element: *mut std::ffi::c_void,
    action: *const std::ffi::c_void,
) -> i32 {
    #[link(name = "ApplicationServices", kind = "framework")]
    extern "C" {
        fn AXUIElementPerformAction(
            element: *mut std::ffi::c_void,
            action: *const std::ffi::c_void,
        ) -> i32;
    }
    AXUIElementPerformAction(element, action)
}

#[cfg(target_os = "macos")]
unsafe fn cf_retain(value: *mut std::ffi::c_void) {
    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        fn CFRetain(cf: *mut std::ffi::c_void) -> *mut std::ffi::c_void;
    }
    let _ = CFRetain(value);
}

#[cfg(target_os = "macos")]
unsafe fn cf_release(value: *mut std::ffi::c_void) {
    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        fn CFRelease(cf: *mut std::ffi::c_void);
    }
    CFRelease(value);
}

#[cfg(target_os = "macos")]
pub(crate) fn has_paste_focus() -> bool {
    PASTE_TARGET_FOCUS.lock().unwrap().is_some()
}

/// Whether Copyosity may use Accessibility APIs (not the frontmost app's AX tree).
#[cfg(target_os = "macos")]
fn accessibility_live_check() -> bool {
    // `AXIsProcessTrusted` is the canonical TCC check. A live AX probe on our own
    // process can return `kAXErrorCannotComplete` (-25208) even when trust is granted,
    // which produced false "not granted" UI while System Settings still showed Copyosity on.
    accessibility_process_trusted()
}

/// `AXIsProcessTrusted` — does not depend on which app is frontmost.
#[cfg(target_os = "macos")]
fn accessibility_process_trusted() -> bool {
    unsafe {
        #[link(name = "ApplicationServices", kind = "framework")]
        extern "C" {
            fn AXIsProcessTrusted() -> bool;
        }
        AXIsProcessTrusted()
    }
}

#[cfg(target_os = "macos")]
fn accessibility_show_prompt() {
    unsafe {
        #[link(name = "ApplicationServices", kind = "framework")]
        extern "C" {
            fn AXIsProcessTrustedWithOptions(options: *const std::ffi::c_void) -> bool;
        }

        use objc2_foundation::{ns_string, NSDictionary, NSNumber};

        let key = ns_string!("AXTrustedCheckOptionPrompt");
        let yes = NSNumber::new_bool(true);
        let dict = NSDictionary::from_slices(&[key], &[&*yes]);
        let _ = AXIsProcessTrustedWithOptions(objc2::rc::Retained::as_ptr(&dict).cast());
    }
    open_accessibility_settings();
}

#[cfg(target_os = "macos")]
pub fn open_accessibility_settings() {
    const URLS: &[&str] = &[
        "x-apple.systempreferences:com.apple.settings.PrivacySecurity.extension?Privacy_Accessibility",
        "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility",
    ];
    for url in URLS {
        if std::process::Command::new("open")
            .arg(url)
            .status()
            .is_ok_and(|s| s.success())
        {
            return;
        }
    }
}

#[cfg(not(target_os = "macos"))]
pub fn open_accessibility_settings() {}

/// Accessibility trust check. `prompt: true` always asks macOS to show its trust dialog.
#[cfg(target_os = "macos")]
pub fn accessibility_trusted(prompt: bool) -> bool {
    if prompt {
        accessibility_show_prompt();
    }
    accessibility_live_check()
}

#[cfg(not(target_os = "macos"))]
pub fn accessibility_trusted(_prompt: bool) -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundle_prefers_keyboard_paste_matches_messages() {
        assert!(bundle_prefers_keyboard_paste("com.apple.MobileSMS"));
        assert!(bundle_prefers_keyboard_paste("com.apple.iChat"));
        assert!(!bundle_prefers_keyboard_paste("com.apple.Notes"));
        assert!(!bundle_prefers_keyboard_paste("com.tinyspeck.slackmacgap"));
    }

    #[test]
    fn bundle_prefers_keyboard_paste_matches_chromium_browsers() {
        assert!(bundle_prefers_keyboard_paste("com.brave.Browser"));
        assert!(bundle_prefers_keyboard_paste("com.google.Chrome"));
        assert!(bundle_prefers_keyboard_paste("org.chromium.Chromium"));
        assert!(bundle_prefers_keyboard_paste("com.microsoft.edgemac"));
        assert!(bundle_prefers_keyboard_paste("com.operasoftware.OperaGX"));
        assert!(bundle_prefers_keyboard_paste("com.google.Chrome.canary"));
        assert!(bundle_prefers_keyboard_paste("com.brave.Browser.beta"));
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn accessibility_trusted_matches_process_trust() {
        assert_eq!(
            accessibility_trusted(false),
            accessibility_process_trusted()
        );
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn editable_role_priority_prefers_text_fields_over_scroll_areas() {
        assert!(
            editable_role_priority("AXTextArea").unwrap()
                < editable_role_priority("AXScrollArea").unwrap()
        );
        assert!(
            editable_role_priority("AXTextField").unwrap()
                < editable_role_priority("AXScrollArea").unwrap()
        );
        assert!(editable_role_priority("AXGroup").is_none());
    }
}
