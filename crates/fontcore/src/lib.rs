//! FontCore: a local, open-source font vault for the storytold (ArtCraft) suite.
//!
//! FontCore never touches the operating system's font registry. Fonts are
//! copied into a vault directory and described by a single `fonts.json`
//! manifest. Management tools mutate the vault through [`Vault`]; suite apps
//! (photocraft, wordcraft, pdfcraft, ...) read it at launch through
//! [`load_for_app`] or [`Vault::fonts_for_app`].
//!
//! ```no_run
//! let set = fontcore::load_for_app("photocraft")?;
//! for font in &set.fonts {
//!     let bytes = font.read()?;
//!     // hand `bytes` to the app's text engine
//! #   let _ = bytes;
//! }
//! # Ok::<(), fontcore::Error>(())
//! ```

mod error;
mod loader;
mod manifest;
mod metadata;
mod vault;

pub use error::{Error, Result};
pub use loader::{load_for_app, FontSet, ResolvedFont};
pub use manifest::{FontEntry, Manifest, MANIFEST_FILE, SCHEMA_VERSION};
pub use metadata::{inspect, FaceInfo, FontFormat};
pub use vault::{AddOptions, AddOutcome, Issue, Vault, FONTS_DIR, HOME_ENV};
