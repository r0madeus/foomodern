//! Standard and enhanced LRC lyrics parser.

use super::LyricLine;

#[derive(Debug, Clone, PartialEq)]
pub enum ParsedLyrics {
    Synced(Vec<LyricLine>),
    Plain(Vec<String>),
}

const META_KEYS: &[&str] = &[
    "ar", "al", "ti", "au", "length", "by", "offset", "re", "tool", "ve", "la",
];

fn parse_timestamp(tag: &str) -> Option<f64> {
    let (mins_str, rest) = tag.split_once(':')?;
    if mins_str.is_empty() || !mins_str.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let mins: u64 = mins_str.parse().ok()?;

    let (secs_str, frac_opt) = match rest.split_once(['.', ':']) {
        Some((s, f)) => (s, Some(f)),
        None => (rest, None),
    };
    if !(1..=2).contains(&secs_str.len()) || !secs_str.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let secs: u64 = secs_str.parse().ok()?;
    if secs >= 60 {
        return None;
    }

    let ms: u64 = match frac_opt {
        None => 0,
        Some(f) => {
            if f.is_empty() || !f.bytes().all(|b| b.is_ascii_digit()) {
                return None;
            }
            let head = &f[..f.len().min(3)];
            let val: u64 = head.parse().ok()?;
            match head.len() {
                1 => val * 100,
                2 => val * 10,
                _ => val,
            }
        }
    };

    Some(mins as f64 * 60.0 + secs as f64 + ms as f64 / 1000.0)
}

fn parse_meta_tag(tag: &str) -> Option<Option<i64>> {
    let (key, val) = tag.split_once(':')?;
    let key = key.trim();
    if !META_KEYS.iter().any(|k| k.eq_ignore_ascii_case(key)) {
        return None;
    }
    let offset = if key.eq_ignore_ascii_case("offset") {
        val.trim().parse::<i64>().ok()
    } else {
        None
    };
    Some(offset)
}

fn strip_inline_timestamps(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find('<') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        if let Some(end) = after.find('>') {
            let inner = &after[..end];
            if parse_timestamp(inner).is_some() {
                rest = &after[end + 1..];
                continue;
            }
        }
        out.push('<');
        rest = after;
    }
    out.push_str(rest);
    out
}

pub fn trim_empty_edges(mut lines: Vec<String>) -> Vec<String> {
    while lines.first().is_some_and(|l| l.is_empty()) {
        lines.remove(0);
    }
    while lines.last().is_some_and(|l| l.is_empty()) {
        lines.pop();
    }
    lines
}

pub fn parse(raw: &str) -> Option<ParsedLyrics> {
    let input = raw.strip_prefix('\u{feff}').unwrap_or(raw);
    let mut offset_ms: i64 = 0;
    let mut raw_timed: Vec<(f64, String)> = Vec::new();
    let mut plain: Vec<String> = Vec::new();

    for line in input.lines() {
        let mut rest = line.trim_start();
        let mut line_times: Vec<f64> = Vec::new();
        let mut has_meta = false;

        while let Some(after_open) = rest.strip_prefix('[') {
            let Some(close) = after_open.find(']') else {
                break;
            };
            let inner = &after_open[..close];
            if let Some(t) = parse_timestamp(inner) {
                line_times.push(t);
                rest = &after_open[close + 1..];
            } else if let Some(maybe_offset) = parse_meta_tag(inner) {
                if let Some(ms) = maybe_offset {
                    offset_ms = ms;
                }
                has_meta = true;
                rest = &after_open[close + 1..];
            } else {
                break;
            }
        }

        let cleaned = strip_inline_timestamps(rest).trim().to_string();
        if !line_times.is_empty() {
            for t in line_times {
                raw_timed.push((t, cleaned.clone()));
            }
        } else if !has_meta || !cleaned.is_empty() {
            plain.push(cleaned);
        }
    }

    if !raw_timed.is_empty() {
        let shift = offset_ms as f64 / 1000.0;
        let mut lines: Vec<LyricLine> = raw_timed
            .into_iter()
            .map(|(t, text)| LyricLine {
                time_seconds: (t - shift).max(0.0),
                text,
            })
            .collect();
        lines.sort_by(|a, b| a.time_seconds.total_cmp(&b.time_seconds));
        return Some(ParsedLyrics::Synced(lines));
    }

    let plain = trim_empty_edges(plain);
    if plain.is_empty() {
        None
    } else {
        Some(ParsedLyrics::Plain(plain))
    }
}
