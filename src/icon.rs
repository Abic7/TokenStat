//! Twin-ring icon renderer. Outer ring = weekly, inner ring = session.
//! Pure pixel maths so it can be unit tested anywhere. Output is straight-alpha RGBA, row 0 at top.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgba(pub u8, pub u8, pub u8, pub u8);

#[cfg(test)]
pub const CLEAR: Rgba = Rgba(0, 0, 0, 0);

pub struct Rings {
    pub outer_pct: Option<f64>,
    pub inner_pct: Option<f64>,
    pub outer: Rgba,
    pub inner: Rgba,
    pub track: Rgba,
    pub background: Rgba,
}

const OUTER_R: f64 = 0.34;
const OUTER_W: f64 = 0.11;
const INNER_R: f64 = 0.19;
const INNER_W: f64 = 0.10;
const SAMPLES: usize = 4;

fn over(top: Rgba, cov: f64, under: [f64; 4]) -> [f64; 4] {
    let a = top.3 as f64 / 255.0 * cov;
    let out_a = a + under[3] * (1.0 - a);
    if out_a <= 0.0 {
        return [0.0; 4];
    }
    let mix = |t: u8, u: f64| (t as f64 * a + u * under[3] * (1.0 - a)) / out_a;
    [mix(top.0, under[0]), mix(top.1, under[1]), mix(top.2, under[2]), out_a]
}

/// Fraction of the ring (0..1) at this angle, measured clockwise from 12 o'clock.
fn turn(dx: f64, dy: f64) -> f64 {
    let a = dx.atan2(-dy) / std::f64::consts::TAU;
    if a < 0.0 {
        a + 1.0
    } else {
        a
    }
}

pub fn render(size: usize, r: &Rings) -> Vec<u8> {
    let s = size as f64;
    let c = s / 2.0;
    let rings = [(OUTER_R * s, OUTER_W * s, r.outer_pct, r.outer), (INNER_R * s, INNER_W * s, r.inner_pct, r.inner)];
    let mut out = Vec::with_capacity(size * size * 4);
    for py in 0..size {
        for px in 0..size {
            let mut bg = 0.0;
            let mut track = [0.0; 2];
            let mut arc = [0.0; 2];
            for sy in 0..SAMPLES {
                for sx in 0..SAMPLES {
                    let x = px as f64 + (sx as f64 + 0.5) / SAMPLES as f64 - c;
                    let y = py as f64 + (sy as f64 + 0.5) / SAMPLES as f64 - c;
                    let d = (x * x + y * y).sqrt();
                    let k = s * 0.22;
                    let (ax, ay) = ((x.abs() - (c - k)).max(0.0), (y.abs() - (c - k)).max(0.0));
                    if (ax * ax + ay * ay).sqrt() <= k {
                        bg += 1.0;
                    }
                    for (i, (radius, width, pct, _)) in rings.iter().enumerate() {
                        if (d - radius).abs() <= width / 2.0 {
                            match pct {
                                Some(p) if turn(x, y) * 100.0 < *p => arc[i] += 1.0,
                                _ => track[i] += 1.0,
                            }
                        }
                    }
                }
            }
            let n = (SAMPLES * SAMPLES) as f64;
            let mut px_col = [0.0; 4];
            px_col = over(r.background, bg / n, px_col);
            for i in 0..2 {
                px_col = over(r.track, track[i] / n, px_col);
                px_col = over(rings[i].3, arc[i] / n, px_col);
            }
            out.extend([px_col[0], px_col[1], px_col[2], px_col[3] * 255.0].map(|v| v.round().clamp(0.0, 255.0) as u8));
        }
    }
    out
}

#[cfg(test)]
pub fn logo() -> Rings {
    Rings {
        outer_pct: Some(75.0),
        inner_pct: Some(50.0),
        outer: Rgba(76, 194, 111, 255),
        inner: Rgba(240, 240, 240, 255),
        track: Rgba(58, 58, 58, 255),
        background: Rgba(31, 31, 31, 255),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Writes raw RGBA logo images for the asset build: `TOKENSTAT_ASSET_DIR=dir cargo test export_logo -- --ignored`
    #[test]
    #[ignore]
    fn export_logo() {
        let dir = std::env::var("TOKENSTAT_ASSET_DIR").unwrap();
        for size in [16, 20, 24, 32, 40, 48, 64, 256, 512] {
            std::fs::write(format!("{dir}/logo-{size}.rgba"), render(size, &logo())).unwrap();
        }
    }

    const GREEN: Rgba = Rgba(76, 194, 111, 255);
    const WHITE: Rgba = Rgba(240, 240, 240, 255);
    const GREY: Rgba = Rgba(58, 58, 58, 255);

    fn rings(o: Option<f64>, i: Option<f64>, bg: Rgba) -> Rings {
        Rings { outer_pct: o, inner_pct: i, outer: GREEN, inner: WHITE, track: GREY, background: bg }
    }

    fn px(img: &[u8], size: usize, x: usize, y: usize) -> [u8; 4] {
        let i = (y * size + x) * 4;
        [img[i], img[i + 1], img[i + 2], img[i + 3]]
    }

    #[test]
    fn size_and_transparent_corners() {
        let img = render(16, &rings(Some(50.0), Some(50.0), CLEAR));
        assert_eq!(img.len(), 16 * 16 * 4);
        assert_eq!(px(&img, 16, 0, 0)[3], 0);
        assert_eq!(px(&img, 16, 8, 8)[3], 0);
    }

    #[test]
    fn arc_fills_clockwise_from_top() {
        let n = 64;
        let img = render(n, &rings(Some(30.0), None, CLEAR));
        let ring_y = (n as f64 / 2.0 - OUTER_R * n as f64) as usize;
        let top_right = px(&img, n, n / 2 + 6, ring_y + 1);
        let top_left = px(&img, n, n / 2 - 6, ring_y + 1);
        assert_eq!(&top_right[..3], &[76, 194, 111]);
        assert_eq!(&top_left[..3], &[58, 58, 58]);
    }

    #[test]
    fn zero_and_full() {
        let n = 64;
        let empty = render(n, &rings(Some(0.0), Some(0.0), CLEAR));
        let full = render(n, &rings(Some(100.0), Some(100.0), CLEAR));
        let y = (n as f64 / 2.0 - OUTER_R * n as f64) as usize + 1;
        assert_eq!(&px(&empty, n, n / 2 - 6, y)[..3], &[58, 58, 58]);
        assert_eq!(&px(&full, n, n / 2 - 6, y)[..3], &[76, 194, 111]);
    }

    #[test]
    fn background_fills_rounded_square() {
        let n = 64;
        let img = render(n, &rings(None, None, Rgba(31, 31, 31, 255)));
        assert_eq!(px(&img, n, n / 2, 0), [31, 31, 31, 255]);
        assert_eq!(px(&img, n, 0, 0)[3], 0);
    }
}
