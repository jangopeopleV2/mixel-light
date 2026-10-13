//! Asset attribution check (AGENTS.md "Assets" rule): every image, icon, font, sound, video, raw
//! file or colour profile in the repository must be covered by a path pattern in the first column of
//! `assets/ATTRIBUTION.md`, licence files referenced there must exist, and nothing may look like it
//! came from an Adobe product.

use std::path::Path;
use std::process::Command;

use sha2::{Digest, Sha256};

/// Immutable two-face UI subset and complete licence bytes from the release's
/// craft-fonts pin. Metadata alone cannot re-attest arbitrary changed files.
const BUNDLED_FONT_PINS: &[(&str, &str)] = &[
    ("fonts/manifest.txt", "ea683de890dfc38307d629de43d7acbe0ced6cdf27982b0e0ab21273665848b9"),
    ("fonts/biz-ud-pgothic/BIZUDPGothic-Regular.ttf", "258d7156c165f2ff774b6efee637c22c3b950de0d8a10e501137061bc8085d01"),
    ("fonts/biz-ud-pgothic/OFL.txt", "e753d7155d53c747d037a445e584c8ecfca6dd79846db610417e282a736b28bc"),
    ("fonts/noto-sans-cjk-sc/NotoSansCJKsc-Regular.otf", "2c76254f6fc379fddfce0a7e84fb5385bb135d3e399294f6eeb6680d0365b74b"),
    ("fonts/noto-sans-cjk-sc/OFL.txt", "6a73f9541c2de74158c0e7cf6b0a58ef774f5a780bf191f2d7ec9cc53efe2bf2"),
];

fn check_pinned_files(directory: &Path, pins: &[(&str, &str)]) -> Vec<String> {
    pins.iter()
        .filter_map(|(name, expected)| {
            let path = directory.join(name);
            match std::fs::read(&path) {
                Ok(bytes) if format!("{:x}", Sha256::digest(&bytes)) == *expected => None,
                Ok(_) => Some(format!("{}: SHA-256 differs from the audited font pin", path.display())),
                Err(error) => Some(format!("{}: pinned font input missing or unreadable: {error}", path.display())),
            }
        })
        .collect()
}

/// Extensions treated as assets (lower-case).
const ASSET_EXT: &[&str] = &[
    "png", "jpg", "jpeg", "gif", "webp", "avif", "svg", "ico", "icns", "bmp", "ttf", "otf", "woff", "woff2", "wav", "mp3", "ogg", "flac", "mp4",
    "mov", "tif", "tiff", "dng", "cr2", "cr3", "nef", "arw", "raf", "orf", "rw2", "rwl", "raw", "pef", "srw", "heic", "jxl", "psd", "cube", "icc",
    "icm", "xmp",
];
/// Adobe-specific formats we never ship (camera/lens profiles, Lightroom templates).
const FORBIDDEN_EXT: &[&str] = &["dcp", "lcp", "lrtemplate", "lrcat", "lrsmcol", "aco", "ase", "abr"];
const FORBIDDEN_NAME: &[&str] = &["adobe", "lightroom", "photoshop", "creative cloud", "creativecloud"];

/// `*` matches any run of characters except `/`; a pattern ending in `/` matches everything below it.
pub fn glob(pat: &str, path: &str) -> bool {
    if let Some(dir) = pat.strip_suffix('/') {
        return path.starts_with(dir) && path[dir.len()..].starts_with('/');
    }
    fn rec(p: &[u8], s: &[u8]) -> bool {
        match (p.first(), s.first()) {
            (None, None) => true,
            (Some(b'*'), _) => rec(&p[1..], s) || (!s.is_empty() && s[0] != b'/' && rec(p, &s[1..])),
            (Some(a), Some(b)) if a == b => rec(&p[1..], &s[1..]),
            _ => false,
        }
    }
    rec(pat.as_bytes(), path.as_bytes())
}

fn backticked(cell: &str) -> impl Iterator<Item = &str> {
    cell.split('`').skip(1).step_by(2)
}

/// (path patterns, referenced licence files) from the attribution table.
pub fn parse(md: &str) -> (Vec<String>, Vec<String>) {
    let (mut pats, mut licences) = (Vec::new(), Vec::new());
    for line in md.lines().filter(|l| l.trim_start().starts_with('|')) {
        let cells: Vec<&str> = line.trim().trim_matches('|').split('|').collect();
        if cells.len() < 5 {
            continue;
        }
        pats.extend(backticked(cells[0]).filter(|p| p.contains('/')).map(str::to_string));
        licences.extend(backticked(cells[4]).filter(|p| p.contains('/')).map(str::to_string));
    }
    (pats, licences)
}

/// Problems with `files` given the attribution document `md`; `exists` checks licence files.
pub fn check(files: &[String], md: &str, exists: impl Fn(&str) -> bool) -> Vec<String> {
    let (pats, licences) = parse(md);
    let mut out = Vec::new();
    for f in files {
        let lower = f.to_ascii_lowercase();
        let ext = Path::new(&lower).extension().and_then(|e| e.to_str()).unwrap_or("");
        let name = Path::new(&lower).file_name().and_then(|e| e.to_str()).unwrap_or("");
        if FORBIDDEN_EXT.contains(&ext) {
            out.push(format!("{f}: `.{ext}` files (Adobe profile/template formats) must never be committed"));
            continue;
        }
        if !ASSET_EXT.contains(&ext) {
            continue;
        }
        if FORBIDDEN_NAME.iter().any(|n| name.contains(n)) {
            out.push(format!("{f}: asset name suggests Adobe material; Adobe assets are forbidden (AGENTS.md)"));
        }
        if !pats.iter().any(|p| glob(p, f)) {
            out.push(format!("{f}: no entry in assets/ATTRIBUTION.md (add path, creator, source, licence, date, modifications)"));
        }
    }
    for l in licences {
        if !exists(&l) {
            out.push(format!("assets/ATTRIBUTION.md references missing licence file `{l}`"));
        }
    }
    out
}

pub fn run(root: &Path) -> Result<(), String> {
    let out = Command::new("git")
        .args(["ls-files", "--cached", "--others", "--exclude-standard"])
        .current_dir(root)
        .output()
        .map_err(|e| format!("git ls-files: {e}"))?;
    if !out.status.success() {
        return Err("git ls-files failed".into());
    }
    let files: Vec<String> = String::from_utf8_lossy(&out.stdout).lines().filter(|f| root.join(f).exists()).map(str::to_string).collect();
    let md = std::fs::read_to_string(root.join("assets/ATTRIBUTION.md")).map_err(|e| format!("assets/ATTRIBUTION.md: {e}"))?;
    let mut problems = check(&files, &md, |p| root.join(p).exists());
    problems.extend(check_pinned_files(&root.join("mixel/vendor/craft-fonts"), BUNDLED_FONT_PINS));
    if problems.is_empty() {
        let n = files
            .iter()
            .filter(|f| Path::new(&f.to_ascii_lowercase()).extension().and_then(|e| e.to_str()).is_some_and(|e| ASSET_EXT.contains(&e)))
            .count();
        eprintln!("assets: {n} asset files, all attributed");
        Ok(())
    } else {
        Err(format!("asset rule violations:\n  {}", problems.join("\n  ")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MD: &str = "| Path | Asset | Creator | Source | Licence | Added | Mods |\n|---|---|---|---|---|---|---|\n\
        | `assets/fonts/Foo-*.ttf` | Foo | A | url | OFL (`assets/fonts/OFL-Foo.txt`) | d | none |\n\
        | `crates/scenes/` | gen | us | original | MIT | d | n/a |\n";

    #[test]
    fn globbing() {
        assert!(glob("docs/images/*-tetons.jpg", "docs/images/ba-tetons.jpg"));
        assert!(!glob("docs/images/*.jpg", "docs/images/sub/x.jpg"));
        assert!(glob("crates/scenes/", "crates/scenes/a/b.png"));
        assert!(!glob("crates/scenes/", "crates/scenesx/b.png"));
    }

    #[test]
    fn flags_unattributed_forbidden_and_missing_licence() {
        let files: Vec<String> =
            ["assets/fonts/Foo-Bold.ttf", "assets/icons/x.svg", "crates/scenes/t.png", "src/lib.rs", "p/camera.dcp", "assets/fonts/Adobe-Foo.otf"]
                .map(String::from)
                .into();
        let p = check(&files, MD, |_| false);
        assert!(p.iter().any(|m| m.starts_with("assets/icons/x.svg: no entry")));
        assert!(p.iter().any(|m| m.starts_with("p/camera.dcp")));
        assert!(p.iter().any(|m| m.contains("Adobe-Foo.otf: asset name")));
        assert!(p.iter().any(|m| m.contains("OFL-Foo.txt")));
        assert!(!p.iter().any(|m| m.starts_with("assets/fonts/Foo-Bold.ttf") || m.starts_with("crates/scenes") || m.starts_with("src/")));
    }

    #[test]
    fn pinned_font_inputs_reject_tampering_and_missing_licences() {
        let dir = std::env::temp_dir().join(format!("lightcraft-font-pin-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("font.txt");
        std::fs::write(&file, b"audited original").unwrap();
        let hash = format!("{:x}", Sha256::digest(b"audited original"));
        assert!(check_pinned_files(&dir, &[("font.txt", &hash)]).is_empty());
        std::fs::write(&file, b"tampered bytes").unwrap();
        assert_eq!(check_pinned_files(&dir, &[("font.txt", &hash)]).len(), 1);
        assert_eq!(check_pinned_files(&dir, &[("OFL.txt", &hash)]).len(), 1);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
