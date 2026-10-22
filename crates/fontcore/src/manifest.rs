use std::fs;
use std::io::Write;
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::IoContext;
use crate::{Error, FaceInfo, FontFormat, Result};

/// File name of the manifest at the root of a vault.
pub const MANIFEST_FILE: &str = "fonts.json";

/// Highest `fonts.json` schema version this build reads and writes.
pub const SCHEMA_VERSION: u32 = 1;

/// The `fonts.json` document: the single source of truth for a vault.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Manifest {
    pub schema_version: u32,
    #[serde(default)]
    pub fonts: Vec<FontEntry>,
}

/// One font file stored in the vault.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FontEntry {
    /// Stable id: the first 16 hex digits of the file's SHA-256.
    pub id: String,
    /// Path of the font file relative to the vault root, using `/` separators.
    pub file: String,
    pub format: FontFormat,
    pub sha256: String,
    pub size: u64,
    pub faces: Vec<FaceInfo>,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    /// Suite apps allowed to load this font. Empty means every app.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub apps: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    /// License identifier or note, e.g. `OFL-1.1`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,
    /// Original file name the font was imported from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    /// Import time in seconds since the Unix epoch.
    pub added_at: u64,
}

fn default_enabled() -> bool {
    true
}

impl FontEntry {
    /// Family name of the first face.
    pub fn family(&self) -> &str {
        self.faces.first().map_or("", |f| f.family.as_str())
    }

    /// Whether `app` should load this font at launch.
    pub fn available_to(&self, app: &str) -> bool {
        self.enabled
            && (self.apps.is_empty() || self.apps.iter().any(|a| a.eq_ignore_ascii_case(app)))
    }

    /// Resolves [`FontEntry::file`] under `root`, rejecting absolute paths and
    /// `..` so a hand-edited manifest cannot point outside the vault.
    pub fn path_in(&self, root: &Path) -> Option<PathBuf> {
        let rel = Path::new(&self.file);
        let safe =
            !self.file.is_empty() && rel.components().all(|c| matches!(c, Component::Normal(_)));
        safe.then(|| root.join(rel))
    }
}

impl Default for Manifest {
    fn default() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            fonts: Vec::new(),
        }
    }
}

impl Manifest {
    pub fn load(path: &Path) -> Result<Self> {
        let text = fs::read_to_string(path).at(path)?;
        let manifest: Self = serde_json::from_str(&text).map_err(|source| Error::Manifest {
            path: path.to_owned(),
            source,
        })?;
        if manifest.schema_version > SCHEMA_VERSION {
            return Err(Error::UnsupportedSchema {
                found: manifest.schema_version,
                supported: SCHEMA_VERSION,
            });
        }
        Ok(manifest)
    }

    /// Writes the manifest atomically so readers never see a partial file.
    pub fn save(&self, path: &Path) -> Result<()> {
        let mut json = serde_json::to_vec_pretty(self).expect("manifest serializes");
        json.push(b'\n');
        write_atomic(path, &json)
    }

    /// Finds an entry by exact id or unique id prefix.
    pub fn find(&self, query: &str) -> Result<usize> {
        if let Some(i) = self.fonts.iter().position(|f| f.id == query) {
            return Ok(i);
        }
        let mut matches = self
            .fonts
            .iter()
            .enumerate()
            .filter(|(_, f)| !query.is_empty() && f.id.starts_with(query));
        match (matches.next(), matches.count()) {
            (Some((i, _)), 0) => Ok(i),
            (Some(_), rest) => Err(Error::Ambiguous(query.into(), rest + 1)),
            (None, _) => Err(Error::NotFound(query.into())),
        }
    }
}

/// Writes `data` to a sibling temp file, syncs it, then renames over `path`.
pub(crate) fn write_atomic(path: &Path, data: &[u8]) -> Result<()> {
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    let mut file = fs::File::create(&tmp).at(&tmp)?;
    file.write_all(data).at(&tmp)?;
    file.sync_all().at(&tmp)?;
    drop(file);
    fs::rename(&tmp, path).at(path)
}
