//! Formatação de números para a interface (tamanhos, velocidades, tempos).

pub fn bytes(n: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut v = n as f64;
    let mut i = 0;
    while v >= 1024.0 && i < UNITS.len() - 1 {
        v /= 1024.0;
        i += 1;
    }
    if i == 0 {
        format!("{n} B")
    } else if v >= 100.0 {
        format!("{} {}", decimal(v, 0), UNITS[i])
    } else {
        format!("{} {}", decimal(v, 1), UNITS[i])
    }
}

pub fn speed(bps: f64) -> String {
    if bps < 1.0 {
        return "0 KB/s".into();
    }
    let s = bytes(bps as u64);
    format!("{s}/s")
}

pub fn duration(secs: u64) -> String {
    if secs >= 99 * 3600 {
        return "∞".into();
    }
    let h = secs / 3600;
    let m = (secs % 3600) / 60;
    let s = secs % 60;
    if h > 0 {
        format!("{h} h {m:02} min")
    } else if m > 0 {
        format!("{m} min {s:02} s")
    } else {
        format!("{s} s")
    }
}

/// Número com vírgula decimal, no jeito brasileiro.
pub fn decimal(v: f64, casas: usize) -> String {
    format!("{v:.casas$}").replace('.', ",")
}

pub fn percent(frac: f32) -> String {
    let p = (frac * 100.0).clamp(0.0, 100.0);
    if p >= 99.95 && frac < 1.0 {
        "99,9%".into()
    } else {
        format!("{}%", decimal(p as f64, if p < 10.0 { 1 } else { 0 }))
    }
}
