//! Dominant colour of an album cover, used by the phone UI to tint the
//! player and lyrics like the cover. Pure and unit tested; the command reads
//! the cached cover file and calls [`dominant_color`].

use image::RgbImage;
use std::path::Path;

/// Side of the thumbnail the colour is computed from.
const SAMPLE: u32 = 48;

/// Most prominent colour of a cover, preferring saturated colours over
/// greys, black and white (which make a poor tint).
pub fn dominant_color(img: &RgbImage) -> [u8; 3] {
    // 4 bits per channel: 4096 buckets, each with a pixel count and sums.
    let mut buckets = vec![(0u32, [0u64; 3]); 4096];
    for p in img.pixels() {
        let [r, g, b] = p.0;
        let idx = ((r as usize >> 4) << 8) | ((g as usize >> 4) << 4) | (b as usize >> 4);
        let bucket = &mut buckets[idx];
        bucket.0 += 1;
        bucket.1[0] += r as u64;
        bucket.1[1] += g as u64;
        bucket.1[2] += b as u64;
    }
    let mut best: Option<(f64, [u8; 3])> = None;
    let mut total = (0u64, [0u64; 3]);
    for (count, sums) in &buckets {
        if *count == 0 {
            continue;
        }
        total.0 += *count as u64;
        for (acc, s) in total.1.iter_mut().zip(sums) {
            *acc += s;
        }
        let avg = sums.map(|s| (s / *count as u64) as u8);
        let (sat, light) = saturation_lightness(avg);
        // Near-black and near-white pixels count far less.
        let tone = if !(0.08..=0.92).contains(&light) {
            0.05
        } else {
            1.0
        };
        let score = *count as f64 * (0.15 + sat) * tone;
        if best.is_none_or(|(s, _)| score > s) {
            best = Some((score, avg));
        }
    }
    match best {
        Some((_, color)) => color,
        None if total.0 > 0 => total.1.map(|s| (s / total.0) as u8),
        None => [0, 0, 0],
    }
}

/// HSL saturation and lightness, both 0..=1.
fn saturation_lightness([r, g, b]: [u8; 3]) -> (f64, f64) {
    let (r, g, b) = (r as f64 / 255.0, g as f64 / 255.0, b as f64 / 255.0);
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let light = (max + min) / 2.0;
    let sat = if max == min {
        0.0
    } else {
        (max - min) / (1.0 - (2.0 * light - 1.0).abs())
    };
    (sat, light)
}

/// Decodes a cover file and returns its dominant colour.
pub fn cover_color(path: &Path) -> Result<[u8; 3], image::ImageError> {
    let img = image::open(path)?.thumbnail(SAMPLE, SAMPLE).into_rgb8();
    Ok(dominant_color(&img))
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgb;

    fn image_of(parts: &[([u8; 3], u32)]) -> RgbImage {
        let total: u32 = parts.iter().map(|p| p.1).sum();
        let mut img = RgbImage::new(total, 1);
        let mut x = 0;
        for (color, n) in parts {
            for _ in 0..*n {
                img.put_pixel(x, 0, Rgb(*color));
                x += 1;
            }
        }
        img
    }

    #[test]
    fn single_colour_is_returned() {
        assert_eq!(
            dominant_color(&image_of(&[([200, 40, 40], 10)])),
            [200, 40, 40]
        );
    }

    #[test]
    fn saturated_colour_beats_a_larger_grey_area() {
        let img = image_of(&[([128, 128, 128], 60), ([30, 140, 200], 40)]);
        assert_eq!(dominant_color(&img), [30, 140, 200]);
    }

    #[test]
    fn white_background_is_ignored() {
        let img = image_of(&[([255, 255, 255], 90), ([220, 120, 100], 10)]);
        assert_eq!(dominant_color(&img), [220, 120, 100]);
    }

    #[test]
    fn all_black_falls_back_to_black() {
        assert_eq!(dominant_color(&image_of(&[([0, 0, 0], 5)])), [0, 0, 0]);
    }

    #[test]
    fn reads_a_png_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("c.png");
        image_of(&[([10, 180, 90], 4)]).save(&path).unwrap();
        assert_eq!(cover_color(&path).unwrap(), [10, 180, 90]);
    }
}
