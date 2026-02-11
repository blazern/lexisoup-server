pub fn truncate(s: &str) -> String {
    const MAX: usize = 512;
    if s.len() > MAX {
        let end = s
            .char_indices()
            .map(|(i, _)| i)
            .take_while(|&i| i <= MAX)
            .last()
            .unwrap_or(MAX);
        format!("{}… ({} bytes)", &s[..end], s.len())
    } else {
        s.to_string()
    }
}
