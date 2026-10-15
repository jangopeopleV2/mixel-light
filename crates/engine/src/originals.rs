//! Never write over an original: exports, renders and other files MIXEL Light writes for the user
//! must not land on a catalogued photo's own file (or its XMP sidecar), whatever the conflict
//! setting or exact path asked for. [`OriginalGuard`] is a snapshot of the library's originals
//! that answers "would writing here replace one?" by file identity (the same file under another
//! spelling, through `..`, a link or a case-insensitive volume counts), not by comparing strings.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use lightcraft_catalog::safe_file::same_file;
use lightcraft_catalog::{Catalog, Source};

/// The library's originals and their sidecars, looked up by file name.
#[derive(Debug, Default)]
pub struct OriginalGuard {
    /// lower-case file name → (path on disk, what it is: "the original of IMG_1.CR3", …)
    by_name: HashMap<String, Vec<(PathBuf, String)>>,
    /// The entries of `by_name` that are symlinks, found on the first check that needs them.
    links: std::sync::OnceLock<Vec<(PathBuf, String)>>,
    /// Missing authorization cannot silently remove an original from the overwrite guard.
    resolution_errors: Vec<String>,
}

impl OriginalGuard {
    /// The originals of every photo in `catalog`.
    pub fn new(catalog: &Catalog) -> Self {
        let mut g = Self::default();
        for p in catalog.photos() {
            if let Source::File { path } = &p.source {
                g.add(Path::new(path), &p.file_name);
            }
        }
        g
    }

    /// Protect `path` (an original named `label`) and its sidecars (`IMG_1.xmp`, `IMG_1.CR3.xmp`).
    pub fn add(&mut self, path: &Path, label: &str) {
        self.add_with(path, label, &|p| crate::macos_mas::resolve(p));
    }

    fn add_with(&mut self, path: &Path, label: &str, resolve: &dyn Fn(&Path) -> Result<PathBuf, String>) {
        self.links = std::sync::OnceLock::new();
        let original = match resolve(path) {
            Ok(path) => path,
            Err(error) => {
                self.resolution_errors.push(format!("Cannot protect the original of {label}: {error}"));
                return;
            }
        };
        let mut candidates = vec![(original.clone(), format!("the original of {label}"))];
        // Include both the resolved original's neighbours and separately bookmarked sidecars
        // under their saved names. A file bookmark can move independently of a folder bookmark.
        for base in [path, original.as_path()] {
            let full = PathBuf::from(format!("{}.xmp", base.to_string_lossy()));
            for sidecar in [base.with_extension("xmp"), base.with_extension("XMP"), full.clone(), full.with_extension("XMP")] {
                candidates.push((sidecar, format!("the XMP sidecar of {label}")));
            }
        }
        for (candidate, what) in candidates {
            let file = match resolve(&candidate) {
                Ok(path) => path,
                Err(error) => {
                    self.resolution_errors.push(format!("Cannot protect {what}: {error}"));
                    continue;
                }
            };
            if crate::macos_mas::enabled()
                && let Err(error) = std::fs::metadata(&file)
                && error.kind() == std::io::ErrorKind::PermissionDenied
            {
                self.resolution_errors.push(format!("Cannot verify {what}: {error}. Choose its containing folder again before exporting."));
            }
            let Some(name) = file.file_name().map(|n| n.to_string_lossy().to_lowercase()) else { continue };
            let entries = self.by_name.entry(name).or_default();
            if !entries.iter().any(|(old, _)| *old == file) {
                entries.push((file, what));
            }
        }
    }

    /// `Err` (a message for the user) when writing `target` would replace a protected file.
    pub fn check(&self, target: &Path) -> Result<(), String> {
        self.check_with(target, &|p| crate::macos_mas::resolve(p))
    }

    fn check_with(&self, target: &Path, resolve: &dyn Fn(&Path) -> Result<PathBuf, String>) -> Result<(), String> {
        if let Some(error) = self.resolution_errors.first() {
            return Err(error.clone());
        }
        let target = resolve(target)?;
        let target = target.as_path();
        if std::fs::symlink_metadata(target).is_err() {
            return Ok(()); // nothing there to replace
        }
        let mut names: Vec<String> = target.file_name().map(|n| n.to_string_lossy().to_lowercase()).into_iter().collect();
        if let Some(n) = std::fs::canonicalize(target).ok().and_then(|c| c.file_name().map(|n| n.to_string_lossy().to_lowercase())) {
            names.push(n);
        }
        for n in &names {
            for (p, what) in self.by_name.get(n).into_iter().flatten() {
                if same_file(p, target) {
                    return Err(format!(
                        "{} is {what} in the library: MIXEL Light never writes over an original (choose another folder or file name)",
                        target.display()
                    ));
                }
            }
        }
        // An imported symlink can have a different name from its original. A write to
        // that original would change what the library reads through the link. Only do
        // this fallback for existing targets that the name index did not protect; the links are
        // looked up once per guard, not once per target.
        let links = self.links.get_or_init(|| self.by_name.values().flatten().filter(|(p, _)| std::fs::read_link(p).is_ok()).cloned().collect());
        for (p, what) in links {
            if same_file(p, target) {
                return Err(format!(
                    "{} is {what} in the library: MIXEL Light never writes over an original (choose another folder or file name)",
                    target.display()
                ));
            }
        }
        Ok(())
    }
}

impl crate::Session {
    /// A guard over this library's originals (see [`OriginalGuard`]).
    pub fn original_guard(&self) -> OriginalGuard {
        OriginalGuard::new(&self.catalog)
    }

    /// `Err` when writing `path` would replace a catalogued photo's original or sidecar. A free
    /// path (the usual case) is answered without building the guard over the whole library.
    pub fn check_write_target(&self, path: &str) -> Result<(), String> {
        let resolved = crate::macos_mas::resolve(path)?;
        if !crate::macos_mas::enabled() && std::fs::symlink_metadata(&resolved).is_err() {
            return Ok(()); // Ordinary desktop free-name fast path.
        }
        self.original_guard().check(&resolved)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lightcraft_catalog::{Op, Photo, PhotoId};

    #[test]
    fn moved_and_renamed_original_and_sidecar_are_still_protected() {
        let dir = std::env::temp_dir().join(format!("lc-originals-bookmark-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let original = dir.join("RENAMED.jpg");
        let sidecar = dir.join("separate-metadata.xmp");
        std::fs::write(&original, b"never overwrite me").unwrap();
        std::fs::write(&sidecar, b"foreign metadata").unwrap();
        let old = Path::new("/old/IMG_1.jpg");
        let resolve = |path: &Path| -> Result<PathBuf, String> {
            if path == old {
                Ok(original.clone())
            } else if path == Path::new("/old/IMG_1.xmp") {
                Ok(sidecar.clone())
            } else {
                Ok(path.to_path_buf())
            }
        };
        let mut guard = OriginalGuard::default();
        guard.add_with(old, "IMG_1.jpg", &resolve);
        assert!(guard.check_with(&original, &resolve).unwrap_err().contains("original"));
        assert!(guard.check_with(old, &resolve).unwrap_err().contains("original"));
        assert!(guard.check_with(&sidecar, &resolve).unwrap_err().contains("sidecar"));
        assert_eq!(std::fs::read(original).unwrap(), b"never overwrite me");
        assert_eq!(std::fs::read(sidecar).unwrap(), b"foreign metadata");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn denied_original_or_target_resolution_fails_closed_even_for_a_missing_target() {
        let denied = |_path: &Path| -> Result<PathBuf, String> { Err("bookmark permission denied".into()) };
        let mut guard = OriginalGuard::default();
        guard.add_with(Path::new("/old/original.jpg"), "original.jpg", &denied);
        assert!(guard.check(Path::new("/new/free-name.jpg")).unwrap_err().contains("permission denied"));
        assert!(OriginalGuard::default().check_with(Path::new("/new/free-name.jpg"), &denied).unwrap_err().contains("permission denied"));
    }

    #[test]
    fn originals_and_their_sidecars_are_protected_by_identity() {
        let d = std::env::temp_dir().join(format!("lc-originals-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("sub")).unwrap();
        let orig = d.join("IMG_1.jpg");
        std::fs::write(&orig, b"original").unwrap();
        std::fs::write(d.join("IMG_1.xmp"), b"<x/>").unwrap();
        std::fs::write(d.join("other.jpg"), b"other").unwrap();
        let mut c = Catalog::default();
        let p = Photo::new(PhotoId(1), Source::File { path: orig.to_string_lossy().to_string() }, "IMG_1.jpg", "JPEG", 10, 10, "2026-01-01");
        c.apply(Op::AddPhoto { photo: Box::new(p) }).unwrap();
        let g = OriginalGuard::new(&c);
        assert!(g.check(&orig).unwrap_err().contains("original of IMG_1.jpg"));
        assert!(g.check(&d.join("sub/../IMG_1.jpg")).is_err(), "another spelling of the same file");
        assert!(g.check(&d.join("IMG_1.xmp")).unwrap_err().contains("sidecar"));
        assert!(g.check(&d.join("other.jpg")).is_ok(), "an ordinary existing file");
        assert!(g.check(&d.join("new.jpg")).is_ok(), "a free name");
        let _ = std::fs::remove_dir_all(&d);
    }
}
