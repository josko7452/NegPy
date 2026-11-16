//! Chart reference colors and CIEDE2000, the fit's error measure.

use negpy_core::mathf::pow_f64;

/// One chart's published colors, row-major in Lab D50.
#[derive(Clone, Debug, PartialEq)]
pub struct ChartSpec {
    pub name: &'static str,
    pub patch_names: &'static [&'static str],
    pub lab_d50: &'static [[f64; 3]],
    pub exposure_patches: &'static [usize],
}

const SPYDER_LAB: [[f64; 3]; 24] = [
    [96.04, 2.16, 2.60],
    [80.44, 1.17, 2.05],
    [65.52, 0.69, 1.86],
    [49.62, 0.58, 1.56],
    [33.55, 0.35, 1.40],
    [16.91, 1.43, -0.81],
    [47.12, -32.50, -28.75],
    [50.49, 53.45, -13.55],
    [83.61, 3.36, 87.02],
    [41.05, 60.75, 31.17],
    [54.14, -40.80, 34.75],
    [24.75, 13.78, -49.48],
    [60.94, 38.21, 61.31],
    [37.80, 7.30, -43.04],
    [49.81, 48.50, 15.76],
    [28.88, 19.36, -24.48],
    [72.45, -23.60, 60.47],
    [71.65, 23.74, 72.28],
    [70.19, -31.90, 1.98],
    [54.38, 8.84, -25.71],
    [42.03, -15.80, 22.93],
    [48.82, -5.11, -23.08],
    [65.10, 18.14, 18.68],
    [36.13, 14.15, 15.78],
];

const SPYDER_NAMES: [&str; 24] = [
    "Card White",
    "20% Gray",
    "40% Gray",
    "60% Gray",
    "80% Gray",
    "Card Black",
    "Primary Cyan",
    "Primary Magenta",
    "Primary Yellow",
    "Primary Red",
    "Primary Green",
    "Primary Blue",
    "Primary Orange",
    "Blueprint",
    "Pink",
    "Violet",
    "Apple Green",
    "Sunflower",
    "Aqua",
    "Lavender",
    "Evergreen",
    "Steel Blue",
    "Classic Light Skin",
    "Classic Dark Skin",
];

const COLORCHECKER_LAB: [[f64; 3]; 24] = [
    [37.54, 14.37, 14.92],
    [64.66, 19.27, 17.50],
    [49.32, -3.82, -22.54],
    [43.46, -12.74, 22.72],
    [54.94, 9.61, -24.79],
    [70.48, -32.26, -0.37],
    [62.73, 35.83, 56.50],
    [39.43, 10.75, -45.17],
    [50.57, 48.64, 16.67],
    [30.10, 22.54, -20.87],
    [71.77, -24.13, 58.19],
    [71.51, 18.24, 67.37],
    [28.37, 15.42, -49.80],
    [54.38, -39.72, 32.27],
    [42.43, 51.05, 28.62],
    [81.80, 2.67, 80.41],
    [50.63, 51.28, -14.12],
    [49.57, -29.71, -28.32],
    [95.19, -1.03, 2.93],
    [81.29, -0.57, 0.44],
    [66.89, -0.75, -0.06],
    [50.76, -0.13, 0.14],
    [35.63, -0.46, -0.48],
    [20.64, 0.07, -0.46],
];

const COLORCHECKER_NAMES: [&str; 24] = [
    "Dark Skin",
    "Light Skin",
    "Blue Sky",
    "Foliage",
    "Blue Flower",
    "Bluish Green",
    "Orange",
    "Purplish Blue",
    "Moderate Red",
    "Purple",
    "Yellow Green",
    "Orange Yellow",
    "Blue",
    "Green",
    "Red",
    "Yellow",
    "Magenta",
    "Cyan",
    "White",
    "Neutral 8",
    "Neutral 6.5",
    "Neutral 5",
    "Neutral 3.5",
    "Black",
];

pub const SPYDERCHECKR_24: ChartSpec = ChartSpec {
    name: "SpyderCheckr 24",
    patch_names: &SPYDER_NAMES,
    lab_d50: &SPYDER_LAB,
    exposure_patches: &[1, 2, 3, 4],
};

pub const COLORCHECKER_24: ChartSpec = ChartSpec {
    name: "ColorChecker 24",
    patch_names: &COLORCHECKER_NAMES,
    lab_d50: &COLORCHECKER_LAB,
    exposure_patches: &[19, 20, 21, 22],
};

pub const CHARTS: [ChartSpec; 2] = [SPYDERCHECKR_24, COLORCHECKER_24];

const D50: [f64; 3] = [0.96422, 1.0, 0.82521];
const D65: [f64; 3] = [0.95047, 1.0, 1.08883];

/// Bradford D50 -> D65 for the chart's published colors.
#[allow(clippy::excessive_precision)]
const D50_TO_D65: [[f64; 3]; 3] = [
    [0.955576615033105, -0.02303934471607878, 0.06316363224980127],
    [-0.02828954424355481, 1.009941617371114, 0.02100765499619033],
    [
        0.012298165717207273,
        -0.02048302523244943,
        1.329909826449757,
    ],
];

fn dot(row: [f64; 3], v: [f64; 3]) -> f64 {
    row[0].mul_add(v[0], row[1].mul_add(v[1], row[2] * v[2]))
}

/// The chart's colors as CIELAB D65.
#[must_use]
pub fn reference_lab(chart: &ChartSpec) -> Vec<[f64; 3]> {
    chart
        .lab_d50
        .iter()
        .map(|&[l, a, b]| {
            let fy = (l + 16.0) / 116.0;
            let f = [fy + a / 500.0, fy, fy - b / 200.0];
            let xyz: [f64; 3] = std::array::from_fn(|i| {
                let f = f[i];
                let v = if f > 6.0 / 29.0 {
                    f * f * f
                } else {
                    (f - 16.0 / 116.0) / 7.787
                };
                v * D50[i]
            });
            let t: [f64; 3] = std::array::from_fn(|i| dot(D50_TO_D65[i], xyz) / D65[i]);
            let g: [f64; 3] = t.map(|v| {
                if v > 0.008856 {
                    pow_f64(v, 1.0 / 3.0)
                } else {
                    7.787 * v + 16.0 / 116.0
                }
            });
            [
                116.0 * g[1] - 16.0,
                500.0 * (g[0] - g[1]),
                200.0 * (g[1] - g[2]),
            ]
        })
        .collect()
}

fn hypot(a: f64, b: f64) -> f64 {
    a.hypot(b)
}

fn degrees(radians: f64) -> f64 {
    radians * (180.0 / std::f64::consts::PI)
}

fn radians(degrees: f64) -> f64 {
    degrees * (std::f64::consts::PI / 180.0)
}

fn cos_deg(value: f64) -> f64 {
    radians(value).cos()
}

fn sin_deg(value: f64) -> f64 {
    radians(value).sin()
}

/// CIEDE2000 between rows of two Lab arrays (Sharma, Wu & Dalal 2005).
#[must_use]
pub fn delta_e2000(lab1: &[[f64; 3]], lab2: &[[f64; 3]]) -> Vec<f64> {
    delta_e2000_with_kl(lab1, lab2, 1.0)
}

/// CIEDE2000 with an explicit lightness weight.
#[must_use]
pub fn delta_e2000_with_kl(lab1: &[[f64; 3]], lab2: &[[f64; 3]], k_l: f64) -> Vec<f64> {
    lab1.iter()
        .zip(lab2)
        .map(|(&[l1, a1, b1], &[l2, a2, b2])| {
            let c_bar = (hypot(a1, b1) + hypot(a2, b2)) / 2.0;
            let c7 = c_bar.powi(7);
            let g = 0.5 * (1.0 - (c7 / (c7 + 25.0f64.powi(7))).sqrt());
            let a1p = (1.0 + g) * a1;
            let a2p = (1.0 + g) * a2;
            let c1p = hypot(a1p, b1);
            let c2p = hypot(a2p, b2);
            let h1 = degrees(b1.atan2(a1p)).rem_euclid(360.0);
            let h2 = degrees(b2.atan2(a2p)).rem_euclid(360.0);
            let mut dh = h2 - h1;
            if dh > 180.0 {
                dh -= 360.0;
            } else if dh < -180.0 {
                dh += 360.0;
            }
            if c1p * c2p == 0.0 {
                dh = 0.0;
            }
            let d_hp = 2.0 * (c1p * c2p).sqrt() * sin_deg(dh / 2.0);
            let l_bar = (l1 + l2) / 2.0;
            let c_bar_p = (c1p + c2p) / 2.0;
            let h_sum = h1 + h2;
            let h_bar = if c1p * c2p == 0.0 {
                h_sum
            } else {
                let h = if (h1 - h2).abs() > 180.0 {
                    if h_sum < 360.0 {
                        h_sum + 360.0
                    } else {
                        h_sum - 360.0
                    }
                } else {
                    h_sum
                };
                h / 2.0
            };
            let t = 1.0 - 0.17 * cos_deg(h_bar - 30.0)
                + 0.24 * cos_deg(2.0 * h_bar)
                + 0.32 * cos_deg(3.0 * h_bar + 6.0)
                - 0.20 * cos_deg(4.0 * h_bar - 63.0);
            let l_delta = l_bar - 50.0;
            let s_l = 1.0 + 0.015 * l_delta * l_delta / (20.0 + l_delta * l_delta).sqrt();
            let s_c = 1.0 + 0.045 * c_bar_p;
            let s_h = 1.0 + 0.015 * c_bar_p * t;
            let c_bar_p7 = c_bar_p.powi(7);
            let r_t = -2.0
                * (c_bar_p7 / (c_bar_p7 + 25.0f64.powi(7))).sqrt()
                * sin_deg(60.0 * (-((h_bar - 275.0) / 25.0).powi(2)).exp());
            let dl = (l2 - l1) / (k_l * s_l);
            let dc = (c2p - c1p) / s_c;
            let dhh = d_hp / s_h;
            (dl * dl + dc * dc + dhh * dhh + r_t * dc * dhh).sqrt()
        })
        .collect()
}

/// CIEDE2000 with the lightness difference left out.
#[must_use]
pub fn delta_e_color(reference: &[[f64; 3]], lab: &[[f64; 3]]) -> Vec<f64> {
    let neutral: Vec<[f64; 3]> = lab
        .iter()
        .zip(reference)
        .map(|(&[_, a, b], &[l, _, _])| [l, a, b])
        .collect();
    delta_e2000(reference, &neutral)
}
