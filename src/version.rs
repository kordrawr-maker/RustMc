pub fn dots(s: &str) -> Option<Vec<u32>> {
    let mut out = Vec::new();
    for part in s.split('.') {
        out.push(part.parse::<u32>().ok()?);
    }
    if out.is_empty() { None } else { Some(out) }
}

pub fn sort_desc<T: AsRef<str>>(v: &mut [T]) {
    v.sort_by_key(|s| std::cmp::Reverse(dots(s.as_ref())));
}

pub fn parse(s: &str) -> Option<(u32, u32, u32)> {
    let d = dots(s)?;
    if d.first() != Some(&1) { return None; }
    Some((1, d.get(1).copied().unwrap_or(0), d.get(2).copied().unwrap_or(0)))
}

pub fn java_major(mc: &str) -> u16 {
    match parse(mc) {
        None => 25,
        Some(v) if v >= (1, 20, 5) => 21,
        Some(_) => 17,
    }
}

pub fn parse_mem(s: &str) -> Option<u64> {
    let s = s.trim();
    let (num, mult) = if let Some(v) = s.strip_suffix('G').or_else(|| s.strip_suffix('g')) {
        (v, 1u64 << 30)
    } else if let Some(v) = s.strip_suffix('M').or_else(|| s.strip_suffix('m')) {
        (v, 1u64 << 20)
    } else if let Some(v) = s.strip_suffix('K').or_else(|| s.strip_suffix('k')) {
        (v, 1u64 << 10)
    } else {
        (s, 1)
    };
    let n: u64 = num.trim().parse().ok()?;
    Some(n.saturating_mul(mult))
}

pub fn human_bytes(n: u64) -> String {
    if n >= 1 << 30 {
        format!("{:.2} GB", n as f64 / (1u64 << 30) as f64)
    } else if n >= 1 << 20 {
        format!("{:.1} MB", n as f64 / (1u64 << 20) as f64)
    } else if n >= 1 << 10 {
        format!("{:.1} KB", n as f64 / (1u64 << 10) as f64)
    } else {
        format!("{n} B")
    }
}

pub fn human_uptime(secs: u64) -> String {
    let d = secs / 86400;
    let h = (secs % 86400) / 3600;
    let m = (secs % 3600) / 60;
    let s = secs % 60;
    if d > 0 {
        format!("{d}d {h:02}:{m:02}:{s:02}")
    } else {
        format!("{h:02}:{m:02}:{s:02}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_mc_versions() {
        assert_eq!(parse("1.21.1"), Some((1, 21, 1)));
        assert_eq!(parse("1.21"), Some((1, 21, 0)));
        assert_eq!(parse("24w14a"), None);
        assert_eq!(parse("2.0.0"), None);
    }

    #[test]
    fn parses_dots() {
        assert_eq!(dots("21.1.115"), Some(vec![21, 1, 115]));
        assert_eq!(dots("0.16.9"), Some(vec![0, 16, 9]));
        assert_eq!(dots("abc"), None);
    }

    #[test]
    fn parses_memory() {
        assert_eq!(parse_mem("2G"), Some(2 * (1 << 30)));
        assert_eq!(parse_mem("512M"), Some(512 * (1 << 20)));
        assert_eq!(parse_mem("2g"), Some(2 * (1 << 30)));
        assert_eq!(parse_mem("junk"), None);
    }

    #[test]
    fn picks_java_major() {
        assert_eq!(java_major("1.16.5"), 17);
        assert_eq!(java_major("1.20.4"), 17);
        assert_eq!(java_major("1.20.5"), 21);
        assert_eq!(java_major("1.21.11"), 21);
        assert_eq!(java_major("26.2"), 25);
    }

    #[test]
    fn formats_bytes() {
        assert_eq!(human_bytes(1 << 30), "1.00 GB");
        assert_eq!(human_bytes(512 * (1 << 20)), "512.0 MB");
        assert_eq!(human_bytes(42), "42 B");
    }
}
