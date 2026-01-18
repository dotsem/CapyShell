#[inline]
pub fn int_to_readable_byte_string(bytes: u64) -> String {
    let (value, unit) = if bytes < 1024 {
        (bytes as f32, "B")
    } else if bytes < 1024 * 1024 {
        (bytes as f32 / 1024.0, "KB")
    } else if bytes < 1024 * 1024 * 1024 {
        (bytes as f32 / (1024.0 * 1024.0), "MB")
    } else {
        (bytes as f32 / (1024.0 * 1024.0 * 1024.0), "GB")
    };

    if unit == "B" {
        format!("{value} {unit}")
    } else {
        format!("{value:.1} {unit}")
    }
}
