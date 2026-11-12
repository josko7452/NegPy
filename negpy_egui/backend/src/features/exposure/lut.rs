//! Print-stock LUT loading from `negpy/features/exposure/print_lut.py`.
//!
//! An NPZ is a ZIP of `.npy` arrays. `npyz` reads the arrays and `zip` reads the archive;
//! both accept the dtypes and shapes the bake writes.

use std::io::{Cursor, Read};
use std::path::Path;

use npyz::NpyFile;
use zip::ZipArchive;

/// One print stock's color: dye densities above base to linear Adobe RGB.
#[derive(Clone, Debug, PartialEq)]
pub struct PrintLut {
    pub key: String,
    /// `(N, N, N, 3)` over C, M, Y density above base, row-major.
    pub table: Vec<f32>,
    pub n: usize,
    pub e_max: [f32; 3],
    pub source: String,
    /// The density 3x3 the Flat master uses in place of the LUT.
    pub flat_rows: Option<[[f64; 3]; 3]>,
}

impl PrintLut {
    /// Load `key` from `dir`, where the bake writes `key.npz`.
    pub fn load(dir: &Path, key: &str) -> Result<Self, String> {
        let path = dir.join(format!("{key}.npz"));
        let data = std::fs::read(&path).map_err(|error| {
            format!("print LUT '{key}' not found at {}: {error}", path.display())
        })?;
        parse(key, &data)
    }
}

/// Parse one NPZ archive.
pub fn parse(key: &str, data: &[u8]) -> Result<PrintLut, String> {
    let mut archive = ZipArchive::new(Cursor::new(data))
        .map_err(|error| format!("open print LUT '{key}': {error}"))?;
    let (table, shape) = read_f32_with_shape(&mut archive, "lut.npy")?;
    let e_max = read_f32(&mut archive, "e_max.npy")?;
    let source = read_text(&mut archive, "source.npy")?;
    if shape.len() != 4 || shape[3] != 3 || shape[0] != shape[1] || shape[1] != shape[2] {
        return Err(format!(
            "print LUT '{key}' has an invalid table shape {shape:?}"
        ));
    }
    let n = shape[0];
    if table.len() != n * n * n * 3 {
        return Err(format!(
            "print LUT '{key}' holds {} values, expected {}",
            table.len(),
            n * n * n * 3
        ));
    }
    if e_max.len() < 3 {
        return Err(format!("print LUT '{key}' has an invalid e_max"));
    }
    let flat_rows = read_f64(&mut archive, "flat.npy")?
        .filter(|flat| flat.len() >= 9)
        .map(|flat| std::array::from_fn(|r| std::array::from_fn(|ch| flat[r * 3 + ch])));
    Ok(PrintLut {
        key: key.to_string(),
        table,
        n,
        e_max: [e_max[0], e_max[1], e_max[2]],
        source,
        flat_rows,
    })
}

fn read_f32(archive: &mut ZipArchive<Cursor<&[u8]>>, name: &str) -> Result<Vec<f32>, String> {
    read_f32_with_shape(archive, name).map(|(values, _)| values)
}

fn read_f32_with_shape(
    archive: &mut ZipArchive<Cursor<&[u8]>>,
    name: &str,
) -> Result<(Vec<f32>, Vec<usize>), String> {
    let bytes = entry_bytes(archive, name)?;
    let file = NpyFile::new(Cursor::new(bytes)).map_err(|error| format!("read {name}: {error}"))?;
    let shape = file.shape().iter().map(|value| *value as usize).collect();
    let values = file
        .into_vec()
        .map_err(|error| format!("read {name}: {error}"))?;
    Ok((values, shape))
}

fn read_f64(
    archive: &mut ZipArchive<Cursor<&[u8]>>,
    name: &str,
) -> Result<Option<Vec<f64>>, String> {
    let Ok(bytes) = entry_bytes(archive, name) else {
        return Ok(None);
    };
    let file = NpyFile::new(Cursor::new(bytes)).map_err(|error| format!("read {name}: {error}"))?;
    file.into_vec()
        .map(Some)
        .map_err(|error| format!("read {name}: {error}"))
}

fn read_text(archive: &mut ZipArchive<Cursor<&[u8]>>, name: &str) -> Result<String, String> {
    let bytes = entry_bytes(archive, name)?;
    let file = NpyFile::new(Cursor::new(bytes)).map_err(|error| format!("read {name}: {error}"))?;
    let text = file
        .into_vec::<Vec<char>>()
        .map_err(|error| format!("read {name}: {error}"))?
        .into_iter()
        .flatten()
        .collect::<String>();
    Ok(text.trim_end_matches('\0').to_string())
}

fn entry_bytes(archive: &mut ZipArchive<Cursor<&[u8]>>, name: &str) -> Result<Vec<u8>, String> {
    let mut entry = archive
        .by_name(name)
        .map_err(|error| format!("print LUT has no {name}: {error}"))?;
    let mut bytes = Vec::with_capacity(entry.size() as usize);
    entry
        .read_to_end(&mut bytes)
        .map_err(|error| format!("read {name}: {error}"))?;
    Ok(bytes)
}
