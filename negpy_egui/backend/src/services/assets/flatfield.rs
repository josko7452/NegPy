//! Mirrors `negpy/services/assets/flatfield.py`: the NPZ profile store.

use std::io::{Cursor, Read, Write};
use std::path::{Path, PathBuf};

use half::f16;
use npyz::NpyFile;
use zip::ZipArchive;

use crate::features::flatfield::logic::{
    Evenness, GainEntry, check_copy, compute_gain, gain_token, to_sensor_layout,
};
use crate::infrastructure::loaders::image_buffer::ImageBuffer;

pub const SENSOR_LAYOUT: &str = "sensor";
const EXT: &str = ".npz";

/// Profile metadata shown in the sidebar.
#[derive(Clone, Debug, PartialEq)]
pub struct FlatFieldProfile {
    pub id: String,
    pub name: String,
    pub k1: f64,
    pub source: String,
}

/// The gain with the stored reference copy and self-check.
#[derive(Clone, Debug, PartialEq)]
pub struct StoredCheck {
    pub gain: ImageBuffer,
    pub reference: Option<ImageBuffer>,
    pub check: Option<Evenness>,
    pub sensor_layout: bool,
}

/// A reference decoded into the profile's gain and check copy.
#[derive(Clone, Debug, PartialEq)]
pub struct Baked {
    pub gain: ImageBuffer,
    pub check: Evenness,
    pub reference: ImageBuffer,
}

/// NPZ files under one flat-field directory.
#[derive(Clone, Debug)]
pub struct FlatFieldProfiles {
    pub directory: PathBuf,
    pub bake: fn(&str) -> Option<Baked>,
}

impl FlatFieldProfiles {
    #[must_use]
    pub fn new(directory: impl Into<PathBuf>) -> Self {
        Self {
            directory: directory.into(),
            bake: bake_gain,
        }
    }

    fn path_for_id(&self, profile_id: &str) -> PathBuf {
        self.directory.join(format!("{profile_id}{EXT}"))
    }

    fn read(
        &self,
        profile_id: &str,
        keys: &[&str],
    ) -> Option<std::collections::HashMap<String, Vec<u8>>> {
        if profile_id.is_empty() {
            return None;
        }
        let data = std::fs::read(self.path_for_id(profile_id)).ok()?;
        let mut archive = ZipArchive::new(Cursor::new(data)).ok()?;
        let mut members = std::collections::HashMap::new();
        for key in keys {
            let name = format!("{key}.npy");
            let Ok(mut entry) = archive.by_name(&name) else {
                continue;
            };
            let mut bytes = Vec::new();
            entry.read_to_end(&mut bytes).ok()?;
            members.insert((*key).to_string(), bytes);
        }
        Some(members)
    }

    fn read_f32(&self, profile_id: &str, key: &str) -> Option<ImageBuffer> {
        let members = self.read(profile_id, &[key])?;
        let (values, shape) = read_f32(&members, key)?;
        let (height, width) = rgb_dims(&shape)?;
        Some(ImageBuffer::from_rgb(width, height, values))
    }

    /// Write one profile file with the optional check copy and self-check.
    pub fn write(
        &self,
        profile_id: &str,
        gain: &ImageBuffer,
        name: &str,
        k1: f64,
        source: &str,
        reference: Option<&ImageBuffer>,
        check: Option<Evenness>,
    ) -> std::io::Result<()> {
        std::fs::create_dir_all(&self.directory)?;
        let mut members: Vec<(&str, NpyArray)> = vec![
            (
                "gain",
                NpyArray::F32(
                    gain.pixels.clone(),
                    vec![gain.height as u64, gain.width as u64, 3],
                ),
            ),
            ("token", NpyArray::Text(gain_token(gain))),
            ("name", NpyArray::Text(name.to_string())),
            ("k1", NpyArray::F64(vec![k1], vec![1])),
            ("source", NpyArray::Text(source.to_string())),
            ("layout", NpyArray::Text(SENSOR_LAYOUT.to_string())),
        ];
        if let Some(reference) = reference {
            let values: Vec<f16> = reference
                .pixels
                .iter()
                .copied()
                .map(f16::from_f32)
                .collect();
            members.push((
                "reference",
                NpyArray::F16(
                    values,
                    vec![reference.height as u64, reference.width as u64, 3],
                ),
            ));
        }
        if let Some(check) = check {
            members.push((
                "check",
                NpyArray::F64(
                    vec![check.low, check.high, check.color, f64::from(check.clipped)],
                    vec![4],
                ),
            ));
        }
        let mut buffer = Cursor::new(Vec::new());
        let mut archive = zip::ZipWriter::new(&mut buffer);
        for (name, array) in members {
            write_array(&mut archive, name, array)?;
        }
        archive.finish()?;
        std::fs::write(self.path_for_id(profile_id), buffer.into_inner())?;
        Ok(())
    }

    /// Bake a reference into a new profile; `None` when the decode fails.
    pub fn create(&self, name: &str, reference_path: &str, k1: f64) -> Option<String> {
        self.create_checked(name, reference_path, k1)
            .map(|(id, _)| id)
    }

    /// `create`, plus the reference's own evenness check.
    pub fn create_checked(
        &self,
        name: &str,
        reference_path: &str,
        k1: f64,
    ) -> Option<(String, Evenness)> {
        let baked = (self.bake)(reference_path)?;
        let profile_id = uuid::Uuid::new_v4().simple().to_string();
        self.write(
            &profile_id,
            &baked.gain,
            name,
            k1,
            reference_path,
            Some(&baked.reference),
            Some(baked.check),
        )
        .ok()?;
        Some((profile_id, baked.check))
    }

    /// Write an already-baked gain map as a new profile.
    pub fn import_gain(&self, gain: &ImageBuffer, name: &str, k1: f64, source: &str) -> String {
        let profile_id = uuid::Uuid::new_v4().simple().to_string();
        let _ = self.write(&profile_id, gain, name, k1, source, None, None);
        profile_id
    }

    /// The gain, its token and layout for the render provider.
    #[must_use]
    pub fn load_gain(&self, profile_id: &str) -> Option<GainEntry> {
        let members = self.read(profile_id, &["gain", "token", "layout"])?;
        let gain = self.read_f32(profile_id, "gain")?;
        let token = read_text(&members, "token").unwrap_or_else(|| gain_token(&gain));
        Some(GainEntry {
            gain,
            token,
            sensor_layout: members.contains_key("layout"),
        })
    }

    /// The gain with the stored reference copy and self-check.
    #[must_use]
    pub fn load_check(&self, profile_id: &str) -> Option<StoredCheck> {
        let members = self.read(profile_id, &["gain", "reference", "check", "layout"])?;
        let gain = self.read_f32(profile_id, "gain")?;
        let reference = if members.contains_key("reference") {
            self.read_f16(profile_id, "reference")
        } else {
            None
        };
        let check = members
            .get("check")
            .and_then(|bytes| NpyFile::new(Cursor::new(bytes.as_slice())).ok())
            .and_then(|file| file.into_vec::<f64>().ok())
            .filter(|values| values.len() >= 4)
            .map(|values| Evenness {
                low: values[0],
                high: values[1],
                color: values[2],
                clipped: values[3] != 0.0,
            });
        Some(StoredCheck {
            gain,
            reference,
            check,
            sensor_layout: members.contains_key("layout"),
        })
    }

    fn read_f16(&self, profile_id: &str, key: &str) -> Option<ImageBuffer> {
        let members = self.read(profile_id, &[key])?;
        let bytes = members.get(key)?;
        let file = NpyFile::new(Cursor::new(bytes.as_slice())).ok()?;
        let shape: Vec<usize> = file.shape().iter().map(|value| *value as usize).collect();
        let (height, width) = rgb_dims(&shape)?;
        let values = file
            .into_vec::<f16>()
            .ok()?
            .into_iter()
            .map(f16::to_f32)
            .collect();
        Some(ImageBuffer::from_rgb(width, height, values))
    }

    /// Profile metadata, read without the gain array.
    #[must_use]
    pub fn get(&self, profile_id: &str) -> Option<FlatFieldProfile> {
        let members = self.read(profile_id, &["name", "k1", "source"])?;
        let name = read_text(&members, "name").unwrap_or_else(|| profile_id.to_string());
        let k1 = members
            .get("k1")
            .and_then(|bytes| NpyFile::new(Cursor::new(bytes.as_slice())).ok())
            .and_then(|file| file.into_vec::<f64>().ok())
            .and_then(|values| values.first().copied())
            .unwrap_or(0.0);
        Some(FlatFieldProfile {
            id: profile_id.to_string(),
            name,
            k1,
            source: read_text(&members, "source").unwrap_or_default(),
        })
    }

    /// `[(id, display name)]`, sorted by name.
    #[must_use]
    pub fn list_profiles(&self) -> Vec<(String, String)> {
        let Ok(entries) = std::fs::read_dir(&self.directory) else {
            return Vec::new();
        };
        let mut out: Vec<(String, String)> = entries
            .filter_map(Result::ok)
            .filter_map(|entry| {
                let name = entry.file_name().to_string_lossy().to_string();
                let id = name.strip_suffix(EXT)?;
                let profile = self.get(id)?;
                Some((profile.id, profile.name))
            })
            .collect();
        out.sort_by_key(|(_, name)| name.to_lowercase());
        out
    }

    /// Delete a profile; a missing file is a no-op.
    pub fn delete(&self, profile_id: &str) {
        let _ = std::fs::remove_file(self.path_for_id(profile_id));
    }
}

impl crate::features::flatfield::logic::GainProvider for FlatFieldProfiles {
    fn gain(&self, profile_id: &str) -> Option<GainEntry> {
        self.load_gain(profile_id)
    }
}

fn rgb_dims(shape: &[usize]) -> Option<(usize, usize)> {
    if shape.len() != 3 || shape[2] != 3 {
        return None;
    }
    Some((shape[0], shape[1]))
}

fn read_f32(
    members: &std::collections::HashMap<String, Vec<u8>>,
    key: &str,
) -> Option<(Vec<f32>, Vec<usize>)> {
    let bytes = members.get(key)?;
    let file = NpyFile::new(Cursor::new(bytes.as_slice())).ok()?;
    let shape = file.shape().iter().map(|value| *value as usize).collect();
    let values = file.into_vec().ok()?;
    Some((values, shape))
}

fn read_text(members: &std::collections::HashMap<String, Vec<u8>>, key: &str) -> Option<String> {
    let bytes = members.get(key)?;
    let file = NpyFile::new(Cursor::new(bytes.as_slice())).ok()?;
    let chars = file.into_vec::<Vec<char>>().ok()?;
    Some(
        chars
            .into_iter()
            .flatten()
            .collect::<String>()
            .trim_end_matches('\0')
            .to_string(),
    )
}

/// Decode a reference like a negative and bake its gain, check copy and self-check.
#[must_use]
pub fn bake_gain(reference_path: &str) -> Option<Baked> {
    let path = Path::new(reference_path);
    if reference_path.is_empty() || !path.exists() {
        return None;
    }
    let loaded = crate::infrastructure::loaders::factory::load(
        path,
        crate::infrastructure::loaders::factory::LoadOptions::new(),
    )
    .ok()?;
    let image = loaded.image?;
    let reference = to_sensor_layout(&image, Some(loaded.orientation));
    let gain = compute_gain(&reference);
    let check = crate::features::flatfield::logic::check_reference(&reference, &gain);
    Some(Baked {
        gain,
        check,
        reference: check_copy(&reference),
    })
}

enum NpyArray {
    F32(Vec<f32>, Vec<u64>),
    F64(Vec<f64>, Vec<u64>),
    F16(Vec<f16>, Vec<u64>),
    Text(String),
}

fn write_array<W: std::io::Write + std::io::Seek>(
    archive: &mut zip::ZipWriter<W>,
    name: &str,
    array: NpyArray,
) -> std::io::Result<()> {
    use npyz::WriterBuilder;

    let entry = zip::write::SimpleFileOptions::default();
    archive.start_file(format!("{name}.npy"), entry)?;
    match array {
        NpyArray::F32(values, shape) => {
            let mut writer = npyz::WriteOptions::new()
                .default_dtype()
                .shape(&shape)
                .writer(archive.by_ref())
                .begin_nd()?;
            writer.extend(values)?;
            writer.finish()?;
        }
        NpyArray::F64(values, shape) => {
            let mut writer = npyz::WriteOptions::new()
                .default_dtype()
                .shape(&shape)
                .writer(archive.by_ref())
                .begin_nd()?;
            writer.extend(values)?;
            writer.finish()?;
        }
        NpyArray::F16(values, shape) => {
            let mut writer = npyz::WriteOptions::new()
                .default_dtype()
                .shape(&shape)
                .writer(archive.by_ref())
                .begin_nd()?;
            writer.extend(values)?;
            writer.finish()?;
        }
        NpyArray::Text(text) => write_text(archive, &text)?,
    }
    Ok(())
}

fn write_text<W: Write + std::io::Seek>(
    archive: &mut zip::ZipWriter<W>,
    text: &str,
) -> std::io::Result<()> {
    use npyz::WriterBuilder;
    let dtype = npyz::DType::parse(&format!("'<U{}'", text.encode_utf16().count()))?;
    let mut buffer = Cursor::new(Vec::new());
    let mut writer = npyz::WriteOptions::new()
        .dtype(dtype)
        .writer(&mut buffer)
        .begin_1d()?;
    writer.push(text)?;
    writer.finish()?;
    archive.write_all(&buffer.into_inner())
}
