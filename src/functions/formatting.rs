pub fn int_to_readable_byte_string(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f32 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.1} MB", bytes as f32 / (1024.0 * 1024.0))
    } else {
        format!("{:.1} GB", bytes as f32 / (1024.0 * 1024.0 * 1024.0))
    }
}
