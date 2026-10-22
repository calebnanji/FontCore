mod common;

use std::fs;
use std::path::{Path, PathBuf};

use fontcore::{inspect, AddOptions, AddOutcome, Error, FontFormat, Issue, Manifest, Vault};
use tempfile::TempDir;

fn write(dir: &Path, name: &str, data: &[u8]) -> PathBuf {
    let path = dir.join(name);
    fs::write(&path, data).unwrap();
    path
}

fn setup() -> (TempDir, Vault) {
    let tmp = TempDir::new().unwrap();
    let vault = Vault::init(tmp.path().join("vault")).unwrap();
    (tmp, vault)
}

#[test]
fn inspect_reads_names() {
    let (format, faces) = inspect(&common::ttf("Story Sans", "Bold", "a")).unwrap();
    assert_eq!(format, FontFormat::Ttf);
    assert_eq!(faces.len(), 1);
    assert_eq!(faces[0].family, "Story Sans");
    assert_eq!(faces[0].subfamily, "Bold");
    assert_eq!(faces[0].full_name.as_deref(), Some("Story Sans Bold"));
    assert_eq!(faces[0].postscript_name.as_deref(), Some("StorySans-Bold"));
}

#[test]
fn inspect_reads_collections() {
    let data = common::ttc(&[
        common::ttf("Craft", "Regular", "a"),
        common::ttf("Craft", "Italic", "b"),
    ]);
    let (format, faces) = inspect(&data).unwrap();
    assert_eq!(format, FontFormat::Ttc);
    let styles: Vec<_> = faces
        .iter()
        .map(|f| (f.index, f.subfamily.as_str()))
        .collect();
    assert_eq!(styles, [(0, "Regular"), (1, "Italic")]);
}

#[test]
fn inspect_rejects_non_fonts() {
    assert!(matches!(
        inspect(b"wOF2...."),
        Err(Error::UnsupportedFormat(_))
    ));
    assert!(matches!(
        inspect(b"hello world"),
        Err(Error::UnsupportedFormat(_))
    ));
    assert!(matches!(
        inspect(&[0, 1, 0, 0, 0, 0]),
        Err(Error::InvalidFont(_))
    ));
}

#[test]
fn open_requires_init() {
    let tmp = TempDir::new().unwrap();
    assert!(matches!(
        Vault::open(tmp.path()),
        Err(Error::NotInitialized(_))
    ));
    Vault::init(tmp.path()).unwrap();
    Vault::init(tmp.path()).unwrap(); // idempotent
    assert!(Vault::open(tmp.path())
        .unwrap()
        .manifest()
        .unwrap()
        .fonts
        .is_empty());
}

#[test]
fn add_copies_and_dedupes() {
    let (tmp, vault) = setup();
    let src = write(
        tmp.path(),
        "Story.ttf",
        &common::ttf("Story", "Regular", "a"),
    );
    let options = AddOptions {
        tags: vec!["display".into()],
        license: Some("OFL-1.1".into()),
        ..Default::default()
    };

    let AddOutcome::Added(entry) = vault.add(&src, &options).unwrap() else {
        panic!("expected a new font");
    };
    assert_eq!(entry.id.len(), 16);
    assert_eq!(entry.file, format!("fonts/{}.ttf", entry.id));
    assert_eq!(entry.source.as_deref(), Some("Story.ttf"));
    assert_eq!(
        fs::read(vault.root().join(&entry.file)).unwrap(),
        fs::read(&src).unwrap()
    );

    // Same bytes under another name are not imported twice.
    let copy = write(tmp.path(), "copy.ttf", &fs::read(&src).unwrap());
    assert!(
        matches!(vault.add(&copy, &options).unwrap(), AddOutcome::AlreadyPresent(e) if e.id == entry.id)
    );

    let manifest = vault.manifest().unwrap();
    assert_eq!(manifest.fonts, vec![entry]);
    assert!(!vault.root().join(".fontcore.lock").exists());
}

#[test]
fn manifest_round_trips_through_json() {
    let (tmp, vault) = setup();
    let src = write(tmp.path(), "a.ttf", &common::ttf("A", "Regular", "a"));
    vault.add(&src, &AddOptions::default()).unwrap();

    let text = fs::read_to_string(vault.manifest_path()).unwrap();
    let json: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(json["schema_version"], 1);
    assert_eq!(json["fonts"][0]["faces"][0]["family"], "A");
    // Empty optional fields are omitted to keep the file readable.
    assert!(json["fonts"][0].get("apps").is_none());
}

#[test]
fn newer_schema_is_rejected() {
    let (_tmp, vault) = setup();
    fs::write(
        vault.manifest_path(),
        r#"{"schema_version": 99, "fonts": []}"#,
    )
    .unwrap();
    assert!(matches!(
        vault.manifest(),
        Err(Error::UnsupportedSchema { found: 99, .. })
    ));
}

#[test]
fn ids_resolve_by_prefix() {
    let (tmp, vault) = setup();
    let a = write(tmp.path(), "a.ttf", &common::ttf("A", "Regular", "a"));
    let id = vault
        .add(&a, &AddOptions::default())
        .unwrap()
        .entry()
        .id
        .clone();
    assert_eq!(vault.get(&id[..4]).unwrap().id, id);
    assert!(matches!(vault.get("zzzz"), Err(Error::NotFound(_))));
    assert!(matches!(vault.get(""), Err(Error::NotFound(_))));

    let mut manifest = vault.manifest().unwrap();
    let mut twin = manifest.fonts[0].clone();
    twin.id = format!("{}0", &id[..4]);
    manifest.fonts.push(twin);
    manifest.save(&vault.manifest_path()).unwrap();
    assert!(matches!(vault.get(&id[..4]), Err(Error::Ambiguous(_, 2))));
}

#[test]
fn remove_deletes_file() {
    let (tmp, vault) = setup();
    let src = write(tmp.path(), "a.ttf", &common::ttf("A", "Regular", "a"));
    let entry = vault
        .add(&src, &AddOptions::default())
        .unwrap()
        .entry()
        .clone();
    vault.remove(&entry.id).unwrap();
    assert!(vault.manifest().unwrap().fonts.is_empty());
    assert!(!vault.root().join(&entry.file).exists());
    assert!(matches!(vault.remove(&entry.id), Err(Error::NotFound(_))));
}

#[test]
fn fonts_for_app_respects_scope_and_state() {
    let (tmp, vault) = setup();
    let add = |name: &str, apps: &[&str]| {
        let src = write(
            tmp.path(),
            &format!("{name}.ttf"),
            &common::ttf(name, "Regular", name),
        );
        let options = AddOptions {
            apps: apps.iter().map(|s| s.to_string()).collect(),
            ..Default::default()
        };
        vault.add(&src, &options).unwrap().entry().id.clone()
    };
    add("Global", &[]);
    add("PhotoOnly", &["photocraft"]);
    let hidden = add("Hidden", &[]);
    vault.set_enabled(&hidden, false).unwrap();

    let families = |app: &str| -> Vec<String> {
        let set = vault.fonts_for_app(app).unwrap();
        assert!(set.unavailable.is_empty());
        set.fonts
            .iter()
            .map(|f| f.entry.family().to_owned())
            .collect()
    };
    assert_eq!(families("photocraft"), ["Global", "PhotoOnly"]);
    assert_eq!(families("PhotoCraft"), ["Global", "PhotoOnly"]);
    assert_eq!(families("wordcraft"), ["Global"]);

    vault.set_enabled(&hidden, true).unwrap();
    assert_eq!(families("wordcraft"), ["Global", "Hidden"]);

    let set = vault.fonts_for_app("pdfcraft").unwrap();
    assert_eq!(
        set.fonts[0].read().unwrap(),
        fs::read(&set.fonts[0].path).unwrap()
    );
}

#[test]
fn loader_reports_missing_and_unsafe_entries() {
    let (tmp, vault) = setup();
    let a = write(tmp.path(), "a.ttf", &common::ttf("A", "Regular", "a"));
    let b = write(tmp.path(), "b.ttf", &common::ttf("B", "Regular", "b"));
    let a = vault
        .add(&a, &AddOptions::default())
        .unwrap()
        .entry()
        .clone();
    vault.add(&b, &AddOptions::default()).unwrap();
    fs::remove_file(vault.root().join(&a.file)).unwrap();

    let mut manifest = vault.manifest().unwrap();
    manifest.fonts[1].file = "../outside.ttf".into();
    manifest.save(&vault.manifest_path()).unwrap();
    fs::write(vault.root().join("../outside.ttf"), b"x").unwrap();

    let set = vault.fonts_for_app("wordcraft").unwrap();
    assert!(set.fonts.is_empty());
    assert_eq!(set.unavailable.len(), 2);
}

#[test]
fn verify_finds_problems() {
    let (tmp, vault) = setup();
    let a = write(tmp.path(), "a.ttf", &common::ttf("A", "Regular", "a"));
    let b = write(tmp.path(), "b.ttf", &common::ttf("B", "Regular", "b"));
    let a = vault
        .add(&a, &AddOptions::default())
        .unwrap()
        .entry()
        .clone();
    let b = vault
        .add(&b, &AddOptions::default())
        .unwrap()
        .entry()
        .clone();
    assert!(vault.verify().unwrap().is_empty());

    fs::remove_file(vault.root().join(&a.file)).unwrap();
    fs::write(vault.root().join(&b.file), b"tampered").unwrap();
    let orphan = write(&vault.root().join("fonts"), "stray.ttf", b"x");

    let issues = vault.verify().unwrap();
    assert_eq!(issues.len(), 3);
    assert!(issues.contains(&Issue::MissingFile {
        id: a.id,
        path: vault.root().join(&a.file)
    }));
    assert!(issues.contains(&Issue::HashMismatch {
        id: b.id,
        path: vault.root().join(&b.file)
    }));
    assert!(issues.contains(&Issue::Orphan { path: orphan }));
}

#[test]
fn stale_lock_is_reclaimed() {
    let (tmp, vault) = setup();
    let lock = vault.root().join(".fontcore.lock");
    let file = fs::File::create(&lock).unwrap();
    let old = std::time::SystemTime::now() - std::time::Duration::from_secs(120);
    file.set_modified(old).unwrap();
    let src = write(tmp.path(), "a.ttf", &common::ttf("A", "Regular", "a"));
    vault.add(&src, &AddOptions::default()).unwrap();
    assert!(!lock.exists());
}

#[test]
fn default_manifest_is_current_schema() {
    assert_eq!(Manifest::default().schema_version, fontcore::SCHEMA_VERSION);
}
