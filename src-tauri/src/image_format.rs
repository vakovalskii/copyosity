use std::path::Path;

/// Detect stored image format from base64 payload magic bytes.
pub fn detect_from_b64(b64: &str) -> &'static str {
    if b64.starts_with("R0lGOD") {
        "GIF"
    } else if b64.starts_with("/9j/") {
        "JPG"
    } else {
        "PNG"
    }
}

/// Detect format from a file path extension (Finder copies).
pub fn detect_from_path(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .as_deref()
    {
        Some("gif") => "GIF",
        Some("png") => "PNG",
        Some("jpg") | Some("jpeg") => "JPG",
        _ => "PNG",
    }
}

/// Canonical display label — JPEG is always normalized to JPG.
pub fn normalize(format: &str) -> &'static str {
    match format.to_ascii_uppercase().as_str() {
        "JPEG" => "JPG",
        "GIF" => "GIF",
        "PNG" => "PNG",
        "JPG" => "JPG",
        _ => "PNG",
    }
}

/// Lowercase tag for clipboard_tags (jpg, not jpeg).
pub fn tag_from_format(format: &str) -> String {
    normalize(format).to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_from_b64_reads_magic() {
        assert_eq!(detect_from_b64("R0lGODlhAQABAIAAAAAAAP"), "GIF");
        assert_eq!(detect_from_b64("iVBORw0KGgoAAAANSUhEUg"), "PNG");
        assert_eq!(detect_from_b64("/9j/4AAQSkZJRgABAQAAAQ"), "JPG");
    }

    #[test]
    fn detect_from_path_maps_extensions() {
        assert_eq!(detect_from_path(Path::new("/tmp/a.gif")), "GIF");
        assert_eq!(detect_from_path(Path::new("/tmp/a.PNG")), "PNG");
        assert_eq!(detect_from_path(Path::new("/tmp/a.jpg")), "JPG");
        assert_eq!(detect_from_path(Path::new("/tmp/a.jpeg")), "JPG");
    }

    #[test]
    fn normalize_maps_jpeg_to_jpg() {
        assert_eq!(normalize("JPEG"), "JPG");
        assert_eq!(normalize("jpeg"), "JPG");
        assert_eq!(normalize("JPG"), "JPG");
        assert_eq!(tag_from_format("JPEG"), "jpg");
    }
}
