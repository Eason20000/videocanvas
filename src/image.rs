//! Image processing utilities: OTSU binarization, ffmpeg frame extraction,
//! and SC-8850 interlace section ordering.

use ffmpeg_next as ffmpeg;
use ffmpeg::util::frame::video::Video;

/// OTSU adaptive threshold: finds optimal binary split for a GRAY8 image.
/// Returns `Vec<bool>` where true = dark pixel (canvas "on").
pub fn otsu_threshold(data: &[u8]) -> Vec<bool> {
    let mut hist = [0u32; 256];
    for &pixel in data {
        hist[pixel as usize] += 1;
    }

    let total = data.len() as f64;
    let mut sum = 0.0f64;
    for (t, &count) in hist.iter().enumerate() {
        sum += t as f64 * count as f64;
    }

    let mut weight_bg = 0.0f64;
    let mut sum_bg = 0.0f64;
    let mut max_var = 0.0f64;
    let mut threshold = 0u8;

    for (t, &count) in hist.iter().enumerate() {
        let h = count as f64;
        weight_bg += h;
        if weight_bg == 0.0 {
            continue;
        }
        let weight_fg = total - weight_bg;
        if weight_fg == 0.0 {
            break;
        }
        sum_bg += t as f64 * h;
        let mean_bg = sum_bg / weight_bg;
        let mean_fg = (sum - sum_bg) / weight_fg;
        let var = weight_bg * weight_fg * (mean_bg - mean_fg) * (mean_bg - mean_fg);
        if var > max_var {
            max_var = var;
            threshold = t as u8;
        }
    }

    data.iter().map(|&p| p <= threshold).collect()
}

/// Floyd-Steinberg error-diffusion dither: converts GRAY8 to binary via
/// midpoint threshold with error propagated to unprocessed neighbors.
/// Returns `Vec<bool>` where true = dark pixel (canvas "on").
pub fn floyd_steinberg_dither(data: &[u8], width: u32, height: u32) -> Vec<bool> {
    let w = width as usize;
    let h = height as usize;
    let mut buf: Vec<i16> = data.iter().map(|&p| p as i16).collect();
    let mut out = vec![false; w * h];

    for y in 0..h {
        for x in 0..w {
            let idx = y * w + x;
            let old = buf[idx];
            let black = old < 128;
            let new: i16 = if black { 0 } else { 255 };
            out[idx] = black;
            let err = old - new;

            if x + 1 < w {
                buf[idx + 1] += err * 7 / 16;
            }
            if y + 1 < h {
                if x > 0 {
                    buf[(y + 1) * w + x - 1] += err * 3 / 16;
                }
                buf[(y + 1) * w + x] += err * 5 / 16;
                if x + 1 < w {
                    buf[(y + 1) * w + x + 1] += err / 16;
                }
            }
        }
    }

    out
}

/// Extract tight-packed GRAY8 pixels from an ffmpeg frame,
/// handling stride alignment when present.
pub fn extract_pixels(frame: &Video) -> Vec<u8> {
    let data = frame.data(0);
    let stride = frame.stride(0);
    let width = frame.width() as usize;
    let height = frame.height() as usize;
    if stride == width {
        return data[..width * height].to_vec();
    }
    let mut pixels = Vec::with_capacity(width * height);
    for row in 0..height {
        pixels.extend_from_slice(&data[row * stride..row * stride + width]);
    }
    pixels
}

/// Return which sections to emit for a given frame.
/// Non-interlace: all 16. Interlace even frame: 9 sections, odd: 8 sections.
pub fn interlace_section_order(frame_index: u64, interlace: bool) -> &'static [usize] {
    const ALL: &[usize] = &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15];
    const EVEN: &[usize] = &[0, 2, 4, 6, 8, 10, 12, 14, 15];
    const ODD: &[usize] = &[1, 3, 5, 7, 9, 11, 13, 15];
    if !interlace {
        return ALL;
    }
    if frame_index.is_multiple_of(2) {
        EVEN
    } else {
        ODD
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_otsu_half_black_half_white() {
        let mut data = vec![0u8; 128];
        data.extend(vec![255u8; 128]);
        let binary = otsu_threshold(&data);
        assert_eq!(binary.len(), 256);
        let trues = binary.iter().filter(|&&b| b).count();
        assert!(trues > 100 && trues < 156);
    }

    #[test]
    fn test_otsu_all_same() {
        let data = vec![128u8; 256];
        let binary = otsu_threshold(&data);
        assert_eq!(binary.len(), 256);
    }

    #[test]
    fn test_interlace_off() {
        let order = interlace_section_order(0, false);
        assert_eq!(order.len(), 16);
        assert_eq!(&order[..], &(0usize..16).collect::<Vec<_>>());
    }

    #[test]
    fn test_interlace_even() {
        let order = interlace_section_order(0, true);
        assert_eq!(order.len(), 9);
        assert_eq!(&order[..], &[0, 2, 4, 6, 8, 10, 12, 14, 15]);
    }

    #[test]
    fn test_interlace_odd() {
        let order = interlace_section_order(1, true);
        assert_eq!(order.len(), 8);
        assert_eq!(&order[..], &[1, 3, 5, 7, 9, 11, 13, 15]);
    }

    #[test]
    fn test_floyd_steinberg_dither_identity() {
        let data = vec![0u8; 100];
        let out = floyd_steinberg_dither(&data, 10, 10);
        assert_eq!(out.len(), 100);
        assert!(out.iter().all(|&b| b));
    }

    #[test]
    fn test_floyd_steinberg_dither_all_white() {
        let data = vec![255u8; 100];
        let out = floyd_steinberg_dither(&data, 10, 10);
        assert_eq!(out.len(), 100);
        assert!(out.iter().all(|&b| !b));
    }
}
