//! Mirrors `negpy/features/exposure/papers.py`: darkroom paper profiles, their tonal
//! overrides, and the density-domain color matrices.

use crate::domain::models::ProcessMode;

pub const DEFAULT_PROFILE_KEY: &str = "neutral";

const IDENTITY_DYE: [[f64; 3]; 3] = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
const DEFAULT_LITH_PATH: [[f64; 2]; 4] = [[4.0, 8.0], [2.0, 8.0], [-1.0, 4.0], [0.0, 0.0]];
const ACHROMATIC_J: [[f64; 3]; 3] = [[1.0 / 3.0; 3]; 3];
const TONAL_DEFAULTS: Tonal = Tonal {
    d_max: 2.3,
    d_min: 0.06,
    toe_sharpness_base: 6.0,
    shoulder_sharpness_base: 3.0,
    toe_height: 0.90,
    shoulder_height: 0.35,
    paper_midtone_gamma: 0.05,
    paper_gamma_width: 0.6,
};

/// A stock's color: a density-domain dye matrix, or the key of a bundled LUT.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PrintColor {
    Matrix([[f64; 3]; 3]),
    Lut(&'static str),
}

/// The tonal keys a profile overrides in the exposure constants.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tonal {
    pub d_max: f64,
    pub d_min: f64,
    pub toe_sharpness_base: f64,
    pub shoulder_sharpness_base: f64,
    pub toe_height: f64,
    pub shoulder_height: f64,
    pub paper_midtone_gamma: f64,
    pub paper_gamma_width: f64,
}

/// One paper's print character.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PaperProfile {
    pub label: &'static str,
    pub short: &'static str,
    /// `"default"`, `"bw"` or `"ra4"`.
    pub kind: &'static str,
    /// `"digital"`, `"paper"` or `"film"`.
    pub medium: &'static str,
    pub tonal: Tonal,
    pub channel_gamma: [f64; 3],
    pub slope_gain: f64,
    pub base_tint_cmy: [f64; 3],
    pub print_color: PrintColor,
    pub lith_path: [[f64; 2]; 4],
}

impl PaperProfile {
    const fn new(
        label: &'static str,
        short: &'static str,
        kind: &'static str,
        medium: &'static str,
    ) -> Self {
        Self {
            label,
            short,
            kind,
            medium,
            tonal: TONAL_DEFAULTS,
            channel_gamma: [1.0; 3],
            slope_gain: 1.0,
            base_tint_cmy: [0.0; 3],
            print_color: PrintColor::Matrix(IDENTITY_DYE),
            lith_path: DEFAULT_LITH_PATH,
        }
    }
}

const fn paper(
    label: &'static str,
    short: &'static str,
    kind: &'static str,
    medium: &'static str,
    d_max: f64,
    d_min: f64,
) -> PaperProfile {
    let mut profile = PaperProfile::new(label, short, kind, medium);
    profile.tonal.d_max = d_max;
    profile.tonal.d_min = d_min;
    profile
}

const fn toe(mut profile: PaperProfile, value: f64) -> PaperProfile {
    profile.tonal.toe_sharpness_base = value;
    profile
}

const fn shoulder(mut profile: PaperProfile, value: f64) -> PaperProfile {
    profile.tonal.shoulder_sharpness_base = value;
    profile
}

const fn gamma(mut profile: PaperProfile, value: f64) -> PaperProfile {
    profile.tonal.paper_midtone_gamma = value;
    profile
}

const fn gamma_width(mut profile: PaperProfile, value: f64) -> PaperProfile {
    profile.tonal.paper_gamma_width = value;
    profile
}

const fn channel_gamma(mut profile: PaperProfile, value: [f64; 3]) -> PaperProfile {
    profile.channel_gamma = value;
    profile
}

const fn gain(mut profile: PaperProfile, value: f64) -> PaperProfile {
    profile.slope_gain = value;
    profile
}

const fn tint(mut profile: PaperProfile, value: [f64; 3]) -> PaperProfile {
    profile.base_tint_cmy = value;
    profile
}

const fn color(mut profile: PaperProfile, value: PrintColor) -> PaperProfile {
    profile.print_color = value;
    profile
}

const fn lith_path(mut profile: PaperProfile, value: [[f64; 2]; 4]) -> PaperProfile {
    profile.lith_path = value;
    profile
}

/// Every bundled paper, in declaration order.
pub static PAPER_PROFILES: &[(&str, PaperProfile)] = &[
    (
        DEFAULT_PROFILE_KEY,
        PaperProfile::new("Digital (Adobe RGB)", "Digital", "default", "digital"),
    ),
    (
        "pakon",
        color(
            PaperProfile::new("Digital (Pakon)", "Pakon", "ra4", "digital"),
            PrintColor::Lut("pakon"),
        ),
    ),
    (
        "frontier",
        color(
            gain(
                gamma_width(
                    gamma(
                        shoulder(
                            toe(
                                paper(
                                    "Digital (Frontier)",
                                    "Frontier",
                                    "ra4",
                                    "digital",
                                    2.52,
                                    0.005,
                                ),
                                3.53,
                            ),
                            1.21,
                        ),
                        0.25,
                    ),
                    2.0,
                ),
                1.2,
            ),
            PrintColor::Lut("frontier"),
        ),
    ),
    (
        "ilford_mg_rc",
        gamma(
            toe(
                paper("Ilford Multigrade RC", "MG RC", "bw", "paper", 2.10, 0.04),
                4.0,
            ),
            0.15,
        ),
    ),
    (
        "ilford_fb_classic",
        gamma(
            toe(
                paper(
                    "Ilford Multigrade FB Classic",
                    "MG FB",
                    "bw",
                    "paper",
                    2.15,
                    0.04,
                ),
                5.0,
            ),
            0.15,
        ),
    ),
    (
        "foma_fomatone",
        lith_path(
            gamma(
                toe(
                    paper(
                        "Foma Fomatone MG Classic",
                        "Fomatone",
                        "bw",
                        "paper",
                        2.0,
                        0.05,
                    ),
                    3.5,
                ),
                0.10,
            ),
            [[14.0, 22.0], [7.0, 26.0], [-8.0, 14.0], [-2.0, 2.0]],
        ),
    ),
    (
        "foma_fomabrom",
        lith_path(
            gamma(
                toe(
                    paper(
                        "Foma Fomabrom Variant",
                        "Fomabrom",
                        "bw",
                        "paper",
                        2.0,
                        0.04,
                    ),
                    4.0,
                ),
                0.15,
            ),
            [[6.0, 18.0], [2.0, 18.0], [-9.0, 10.0], [-2.0, 1.0]],
        ),
    ),
    (
        "kodak_polymax_ii",
        gamma_width(
            gamma(
                shoulder(
                    toe(
                        paper(
                            "Kodak Polymax II RC",
                            "Polymax II",
                            "bw",
                            "paper",
                            2.13,
                            0.062,
                        ),
                        3.58,
                    ),
                    3.07,
                ),
                -0.111,
            ),
            0.47,
        ),
    ),
    (
        "kodak_polycontrast_iv",
        gamma_width(
            gamma(
                shoulder(
                    toe(
                        paper(
                            "Kodak Polycontrast IV RC",
                            "Polycontrast IV",
                            "bw",
                            "paper",
                            2.15,
                            0.062,
                        ),
                        5.99,
                    ),
                    2.96,
                ),
                -0.148,
            ),
            0.85,
        ),
    ),
    (
        "kodak_endura",
        color(
            channel_gamma(
                gamma(
                    toe(
                        paper(
                            "Kodak Endura Premier",
                            "Endura Premier",
                            "ra4",
                            "paper",
                            2.55,
                            0.06,
                        ),
                        3.5,
                    ),
                    0.22,
                ),
                [1.04, 1.0, 0.98],
            ),
            PrintColor::Lut("kodak_e4070"),
        ),
    ),
    (
        "fuji_crystal",
        color(
            tint(
                gamma(
                    shoulder(
                        toe(
                            paper(
                                "Fujicolor Crystal Archive",
                                "Crystal Archive",
                                "ra4",
                                "paper",
                                2.42,
                                0.083,
                            ),
                            5.96,
                        ),
                        2.89,
                    ),
                    0.22,
                ),
                [0.008, 0.007, -0.016],
            ),
            PrintColor::Lut("fuji_ca_dpii"),
        ),
    ),
    (
        "fuji_crystal_hd",
        color(
            tint(
                gamma(
                    shoulder(
                        toe(
                            paper(
                                "Fujicolor Crystal Archive HD",
                                "Crystal HD",
                                "ra4",
                                "paper",
                                2.14,
                                0.071,
                            ),
                            5.96,
                        ),
                        2.89,
                    ),
                    0.22,
                ),
                [0.015, 0.011, -0.026],
            ),
            PrintColor::Lut("fuji_ca_hd"),
        ),
    ),
    (
        "kodak_portra_endura",
        color(
            tint(
                gamma_width(
                    gamma(
                        shoulder(
                            toe(
                                channel_gamma(
                                    paper(
                                        "Kodak Portra Endura",
                                        "Portra Endura",
                                        "ra4",
                                        "paper",
                                        2.65,
                                        0.093,
                                    ),
                                    [0.968, 1.0, 1.013],
                                ),
                                1.78,
                            ),
                            2.81,
                        ),
                        0.25,
                    ),
                    0.89,
                ),
                [-0.002, 0.012, -0.009],
            ),
            PrintColor::Lut("kodak_e4021"),
        ),
    ),
    (
        "kodak_supra_endura",
        color(
            tint(
                gamma_width(
                    gamma(
                        shoulder(
                            toe(
                                channel_gamma(
                                    paper(
                                        "Kodak Supra Endura",
                                        "Supra Endura",
                                        "ra4",
                                        "paper",
                                        2.60,
                                        0.095,
                                    ),
                                    [1.008, 1.0, 1.0],
                                ),
                                7.99,
                            ),
                            3.23,
                        ),
                        0.165,
                    ),
                    0.62,
                ),
                [0.011, -0.009, -0.003],
            ),
            PrintColor::Lut("kodak_e4021"),
        ),
    ),
    (
        "kodak_ultra_endura",
        color(
            tint(
                gamma_width(
                    gamma(
                        shoulder(
                            toe(
                                channel_gamma(
                                    paper(
                                        "Kodak Ultra Endura",
                                        "Ultra Endura",
                                        "ra4",
                                        "paper",
                                        2.61,
                                        0.096,
                                    ),
                                    [1.109, 1.0, 1.051],
                                ),
                                6.04,
                            ),
                            5.74,
                        ),
                        0.197,
                    ),
                    0.60,
                ),
                [-0.003, -0.003, 0.005],
            ),
            PrintColor::Lut("kodak_e4070"),
        ),
    ),
    (
        "kodak_ektacolor_edge",
        color(
            tint(
                gamma_width(
                    gamma(
                        shoulder(
                            toe(
                                channel_gamma(
                                    paper(
                                        "Kodak Ektacolor Edge",
                                        "Ektacolor Edge",
                                        "ra4",
                                        "paper",
                                        2.25,
                                        0.083,
                                    ),
                                    [1.013, 1.0, 1.004],
                                ),
                                2.30,
                            ),
                            2.34,
                        ),
                        0.250,
                    ),
                    0.82,
                ),
                [0.011, 0.011, -0.022],
            ),
            PrintColor::Lut("kodak_e4070"),
        ),
    ),
    (
        "kodak_2383",
        color(
            channel_gamma(
                gamma_width(
                    gamma(
                        shoulder(
                            toe(
                                paper("Kodak Vision 2383", "2383", "ra4", "film", 2.29, 0.065),
                                2.65,
                            ),
                            2.86,
                        ),
                        -0.019,
                    ),
                    0.31,
                ),
                [1.034, 1.0, 0.989],
            ),
            PrintColor::Lut("kodak_2383"),
        ),
    ),
    (
        "kodak_2393",
        color(
            channel_gamma(
                gamma_width(
                    gamma(
                        shoulder(
                            toe(
                                paper("Kodak Vision 2393", "2393", "ra4", "film", 2.25, 0.025),
                                3.22,
                            ),
                            2.28,
                        ),
                        0.074,
                    ),
                    0.58,
                ),
                [1.075, 1.0, 1.049],
            ),
            PrintColor::Lut("kodak_2393"),
        ),
    ),
];

/// The profile for `key`, falling back to the Digital default on unknown keys.
#[must_use]
pub fn resolve_paper(key: &str) -> &'static PaperProfile {
    PAPER_PROFILES
        .iter()
        .find_map(|(name, profile)| (*name == key).then_some(profile))
        .unwrap_or(&PAPER_PROFILES[0].1)
}

/// The paper's straight-line slope over the one its Grade gives; 1 without a paper.
#[must_use]
pub fn slope_gain(paper: Option<&PaperProfile>) -> f64 {
    paper.map_or(1.0, |profile| profile.slope_gain)
}

/// The tonal constants with the profile's overrides applied.
#[must_use]
pub fn effective_constants(paper: Option<&PaperProfile>) -> Tonal {
    match paper {
        Some(profile) if profile.kind != "default" => profile.tonal,
        _ => TONAL_DEFAULTS,
    }
}

const MODE_KIND: [(ProcessMode, &str); 2] = [(ProcessMode::C41, "ra4"), (ProcessMode::BW, "bw")];
const MEDIUM_ORDER: [&str; 3] = ["digital", "paper", "film"];

/// Selectable (key, profile) pairs for `mode`: the Digital default first, then matching
/// papers by medium and label. E-6 gets the default only.
#[must_use]
pub fn profiles_for_mode(mode: ProcessMode) -> Vec<(&'static str, &'static PaperProfile)> {
    let mut out = vec![(PAPER_PROFILES[0].0, &PAPER_PROFILES[0].1)];
    let Some((_, allowed)) = MODE_KIND.iter().find(|(candidate, _)| *candidate == mode) else {
        return out;
    };
    let mut papers: Vec<_> = PAPER_PROFILES
        .iter()
        .skip(1)
        .filter(|(_, profile)| profile.kind == *allowed)
        .map(|(key, profile)| (*key, profile))
        .collect();
    papers.sort_by_key(|(_, profile)| {
        (
            MEDIUM_ORDER
                .iter()
                .position(|medium| *medium == profile.medium)
                .unwrap_or(MEDIUM_ORDER.len()),
            profile.label.to_lowercase(),
        )
    });
    out.extend(papers);
    out
}

/// Mode-aware resolve: the stored profile only when its kind matches the mode, otherwise the
/// Digital default.
#[must_use]
pub fn effective_paper_profile(key: &str, mode: Option<ProcessMode>) -> &'static PaperProfile {
    let profile = resolve_paper(key);
    if profile.kind == "default" {
        return profile;
    }
    let matching = mode.and_then(|mode| {
        MODE_KIND
            .iter()
            .find_map(|(candidate, kind)| (*candidate == mode).then_some(*kind))
    });
    if matching == Some(profile.kind) {
        profile
    } else {
        &PAPER_PROFILES[0].1
    }
}

/// The row-normalized dye matrix for a matrix stock, or its LUT key. None for identity, so
/// the default path stays byte-exact.
#[must_use]
pub fn resolve_print_color(
    paper: Option<&PaperProfile>,
) -> (Option<[[f64; 3]; 3]>, Option<&'static str>) {
    match paper.map(|profile| profile.print_color) {
        Some(PrintColor::Lut(key)) => (None, Some(key)),
        None | Some(PrintColor::Matrix(IDENTITY_DYE)) => (None, None),
        Some(PrintColor::Matrix(rows)) => {
            let normalized = std::array::from_fn(|r| {
                let sum = rows[r].iter().sum::<f64>().max(1e-6);
                rows[r].map(|value| value / sum)
            });
            (Some(normalized), None)
        }
    }
}

/// The stock's color as a density 3x3 for the Flat master. A LUT stock's baked fallback is
/// not loaded here.
#[must_use]
pub fn resolve_flat_matrix(paper: Option<&PaperProfile>) -> Option<[[f64; 3]; 3]> {
    resolve_print_color(paper).0
}

/// Density-domain saturation matrix. Row `ch` is `k[ch] * I + (1 - k[ch]) * J`, so each
/// row still sums to 1. Identity returns None.
#[must_use]
pub fn resolve_saturation_matrix(k_rgb: [f64; 3]) -> Option<[[f64; 3]; 3]> {
    (k_rgb != [1.0; 3]).then(|| {
        std::array::from_fn(|ch| {
            std::array::from_fn(|column| {
                if ch == column {
                    k_rgb[ch] + (1.0 - k_rgb[ch]) * ACHROMATIC_J[ch][column]
                } else {
                    (1.0 - k_rgb[ch]) * ACHROMATIC_J[ch][column]
                }
            })
        })
    })
}

/// Composite density-domain matrix for the dye-mix slot: saturation outermost, dye
/// innermost. None when both factors are identity.
#[must_use]
pub fn compose_density_matrices(
    dye: Option<[[f64; 3]; 3]>,
    sat: Option<[[f64; 3]; 3]>,
) -> Option<[[f64; 3]; 3]> {
    if dye.is_none() && sat.is_none() {
        return None;
    }
    let d = dye.unwrap_or(IDENTITY_DYE);
    let s = sat.unwrap_or(IDENTITY_DYE);
    Some(std::array::from_fn(|r| {
        std::array::from_fn(|c| (0..3).map(|k| s[r][k] * d[k][c]).sum())
    }))
}
