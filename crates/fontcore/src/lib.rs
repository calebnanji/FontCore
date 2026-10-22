//! FontCore: a local, open-source font vault for the storytold (ArtCraft) suite.
//!
//! FontCore never touches the operating system's font registry. Fonts are
//! copied into a vault directory and described by a single `fonts.json`
//! manifest. Management tools mutate the vault through [`Vault`]; suite apps
//! (photocraft, wordcraft, pdfcraft, ...) read it at launch through
//! [`load_for_app`] or [`Vault::fonts_for_app`].
//!
//! ```no_run
//! let set = fontcore::load_for_app("ai.storytold.photocraft")?;
//! for font in &set.fonts {
//!     let bytes = font.read()?;
//!     for face in font.faces() {
//!         // register face.index of `bytes` with the app's text engine
//! #       let _ = (&bytes, face);
//!     }
//! }
//! # Ok::<(), fontcore::Error>(())
//! ```

mod app_id;
mod error;
mod license;
mod loader;
mod manifest;
mod metadata;
mod timestamp;
mod vault;

pub use app_id::AppId;
pub use error::{Error, Result};
pub use license::{CustomLicense, License};
pub use loader::{load_for_app, FontSet, ResolvedFont};
pub use manifest::{FontEntry, Manifest, MANIFEST_FILE, SCHEMA_VERSION};
pub use metadata::{inspect, FaceInfo, FontFormat};
pub use timestamp::Timestamp;
pub use vault::{AddOptions, AddOutcome, Issue, Vault, FONTS_DIR, HOME_ENV};
