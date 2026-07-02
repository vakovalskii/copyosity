//! Native macOS integrations exposed to the agent as tools: Notes, Reminders,
//! Calendar. Implemented via `osascript` (Apple Events). Requires the
//! com.apple.security.automation.apple-events entitlement + NSAppleEventsUsageDescription
//! under hardened runtime; first use triggers a per-app Automation prompt.

#[cfg(target_os = "macos")]
fn run_osascript(script: &str, args: &[&str]) -> Result<String, String> {
    use std::io::Write;
    use std::process::{Command, Stdio};

    let mut cmd = Command::new("/usr/bin/osascript");
    cmd.arg("-"); // read script from stdin; user text passed as argv
    for a in args {
        cmd.arg(a);
    }
    cmd.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = cmd.spawn().map_err(|e| format!("spawn osascript: {e}"))?;
    child
        .stdin
        .take()
        .ok_or("no stdin")?
        .write_all(script.as_bytes())
        .map_err(|e| format!("write script: {e}"))?;
    let out = child.wait_with_output().map_err(|e| e.to_string())?;

    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
    } else {
        let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
        if err.contains("-1743") {
            Err("Automation access not granted. Allow Copyosity in System Settings → Privacy & Security → Automation.".to_string())
        } else {
            Err(err)
        }
    }
}

#[cfg(target_os = "macos")]
fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Create a Note. Notes uses the first line of the HTML body as the title.
#[cfg(target_os = "macos")]
pub fn create_note(title: &str, body: &str) -> Result<String, String> {
    let html = format!(
        "<h1>{}</h1><div>{}</div>",
        html_escape(title),
        html_escape(body).replace('\n', "<br>")
    );
    // Body passed as argv item 1 so the shell handles all quoting/escaping.
    let script = r#"on run argv
    set theBody to item 1 of argv
    tell application "Notes" to make new note with properties {body:theBody}
    return "Note created"
end run"#;
    run_osascript(script, &[&html])
}

/// Create a Reminder. `due_offset_secs` = seconds from now (None = no due date).
#[cfg(target_os = "macos")]
pub fn create_reminder(name: &str, due_offset_secs: Option<i64>) -> Result<String, String> {
    let due_clause = match due_offset_secs {
        Some(secs) if secs > 0 => format!("set due date of newR to (current date) + {}", secs),
        _ => String::new(),
    };
    let script = format!(
        r#"on run argv
    set theName to item 1 of argv
    tell application "Reminders"
        set newR to make new reminder with properties {{name:theName}}
        {due}
    end tell
    return "Reminder created"
end run"#,
        due = due_clause
    );
    run_osascript(&script, &[name])
}

/// List incomplete reminders (one per line).
#[cfg(target_os = "macos")]
pub fn list_reminders() -> Result<String, String> {
    let script = r#"tell application "Reminders"
    set out to {}
    repeat with r in (every reminder whose completed is false)
        set end of out to (name of r as text)
    end repeat
    set AppleScript's text item delimiters to linefeed
    return out as text
end tell"#;
    let res = run_osascript(script, &[])?;
    if res.is_empty() {
        Ok("(no open reminders)".to_string())
    } else {
        Ok(res)
    }
}

/// Read upcoming Calendar events within `days` (one per line: "title — when").
#[cfg(target_os = "macos")]
pub fn read_calendar(days: i64) -> Result<String, String> {
    let days = days.clamp(1, 60);
    let script = format!(
        r#"set theStart to (current date)
set theEnd to theStart + ({} * days)
set out to {{}}
tell application "Calendar"
    repeat with cal in calendars
        repeat with e in (every event of cal whose start date is greater than or equal to theStart and start date is less than or equal to theEnd)
            set end of out to ((summary of e) & " — " & (start date of e as text))
        end repeat
    end repeat
end tell
set AppleScript's text item delimiters to linefeed
return out as text"#,
        days
    );
    let res = run_osascript(&script, &[])?;
    if res.is_empty() {
        Ok("(no events in range)".to_string())
    } else {
        Ok(res)
    }
}

// ---- non-macOS stubs ----
#[cfg(not(target_os = "macos"))]
pub fn create_note(_t: &str, _b: &str) -> Result<String, String> {
    Err("macOS only".into())
}
#[cfg(not(target_os = "macos"))]
pub fn create_reminder(_n: &str, _d: Option<i64>) -> Result<String, String> {
    Err("macOS only".into())
}
#[cfg(not(target_os = "macos"))]
pub fn list_reminders() -> Result<String, String> {
    Err("macOS only".into())
}
#[cfg(not(target_os = "macos"))]
pub fn read_calendar(_d: i64) -> Result<String, String> {
    Err("macOS only".into())
}
