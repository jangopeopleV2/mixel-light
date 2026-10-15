//! Exercise the production batch boundary before any renderer/reader, collision probe or writer.
//! Injected policies describe a host's existing authority; they never create native bookmarks.

use super::*;
use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Default)]
struct Calls {
    entered: Cell<usize>,
    progress: Cell<usize>,
    collisions: Cell<usize>,
    writes: RefCell<Vec<(String, Vec<u8>)>>,
}

impl Calls {
    fn untouched(&self) {
        assert_eq!(self.entered.get(), 0, "batch execution must not start before full preflight");
        assert_eq!(self.progress.get(), 0);
        assert_eq!(self.collisions.get(), 0, "denial precedes even destination existence probes");
        assert!(self.writes.borrow().is_empty());
    }
}

fn folder() -> PathBuf {
    std::env::current_dir().unwrap().join("__mixel_light_export_access_fixture__")
}

fn text(path: &Path) -> String {
    path.to_str().unwrap().to_owned()
}

fn identity(path: &Path) -> Result<PathBuf, String> {
    Ok(path.to_path_buf())
}

fn rendered(count: usize, options: &ExportOptions) -> Vec<PreparedExport> {
    let mut session = crate::Session::with_demo();
    let ids = session.visible().iter().copied().take(count).collect::<Vec<_>>();
    assert_eq!(ids.len(), count);
    prepare_batch(&mut session, &ids, options).unwrap()
}

fn png() -> ExportOptions {
    ExportOptions::from_json(&serde_json::json!({"format": "png", "width": 8}))
}

fn execute(
    items: Vec<PreparedExport>,
    options: &ExportOptions,
    destination: &Destination,
    access: BatchAccess<'_>,
    calls: &Calls,
) -> Result<Vec<serde_json::Value>, String> {
    with_batch_destination(items, options, destination, access, |items, destination| {
        calls.entered.set(calls.entered.get() + 1);
        let mut write = |path: &str, bytes: &[u8]| {
            calls.writes.borrow_mut().push((path.to_owned(), bytes.to_vec()));
            Ok(())
        };
        let exists = |_: &str| {
            calls.collisions.set(calls.collisions.get() + 1);
            false
        };
        let mut progress = |_: usize, _: &str| {
            calls.progress.set(calls.progress.get() + 1);
            true
        };
        run_batch_with(items, &mut Placer::new(options, &destination, &mut write, &exists, true), &mut progress, 1)
    })
}

#[test]
fn multi_photo_directory_denial_precedes_every_render_collision_and_write() {
    let options = png();
    let calls = Calls::default();
    let destination = Destination { dir: text(&folder()), exact: None };
    let result = execute(
        rendered(2, &options),
        &options,
        &destination,
        BatchAccess { sandboxed: true, resolve: &identity, directory_granted: &|_| false },
        &calls,
    );
    assert!(result.unwrap_err().contains("folder picker"));
    calls.untouched();
}

#[test]
fn a_single_derived_name_still_requires_the_explicit_folder() {
    let options = png();
    let calls = Calls::default();
    let destination = Destination { dir: text(&folder()), exact: None };
    let result = execute(
        rendered(1, &options),
        &options,
        &destination,
        BatchAccess { sandboxed: true, resolve: &identity, directory_granted: &|_| false },
        &calls,
    );
    assert!(result.unwrap_err().contains("folder picker"));
    calls.untouched();
}

#[test]
fn the_last_resolved_output_is_checked_before_any_earlier_photo_can_write() {
    let options = png();
    let items = rendered(2, &options);
    let denied_name = items[1].file_name.clone();
    let root = folder();
    let elsewhere = root.with_file_name("ungranted-output");
    let resolve = |path: &Path| {
        Ok(if path.file_name().is_some_and(|name| name == denied_name.as_str()) { elsewhere.join(&denied_name) } else { path.to_path_buf() })
    };
    let granted = |path: &Path| path.starts_with(&root);
    let calls = Calls::default();
    let result = execute(
        items,
        &options,
        &Destination { dir: text(&root), exact: None },
        BatchAccess { sandboxed: true, resolve: &resolve, directory_granted: &granted },
        &calls,
    );
    assert!(result.unwrap_err().contains("ungranted-output"));
    calls.untouched();
}

#[test]
fn explicit_ancestor_scope_covers_captured_names_nested_subfolder_and_unique_suffixes() {
    let mut options = png();
    options.naming = "same".into();
    options.subfolder = "album/2026".into();
    options.conflict = Conflict::Unique;
    let root = folder();
    let granted = |path: &Path| path.starts_with(&root);
    let calls = Calls::default();
    let results = execute(
        rendered(2, &options),
        &options,
        &Destination { dir: text(&root), exact: None },
        BatchAccess { sandboxed: true, resolve: &identity, directory_granted: &granted },
        &calls,
    )
    .unwrap();
    assert_eq!(results.len(), 2);
    assert_eq!(calls.entered.get(), 1);
    assert_eq!(calls.progress.get(), 2);
    let writes = calls.writes.borrow();
    assert_eq!(writes.len(), 2);
    for (path, bytes) in writes.iter() {
        assert_eq!(Path::new(path).parent(), Some(root.join("album").join("2026").as_path()));
        assert!(bytes.starts_with(b"\x89PNG\r\n\x1a\n"), "the real renderer/PNG encoder must run after consent");
    }
    assert_eq!(Path::new(&writes[0].0).file_name().unwrap(), "same.png");
    assert_eq!(Path::new(&writes[1].0).file_name().unwrap(), "same-2.png");
}

#[test]
fn captured_filename_tokens_are_sanitized_under_the_granted_subfolder() {
    let options = ExportOptions { naming: "set/{seq}\\{name}".into(), subfolder: "album".into(), ..png() };
    let root = folder();
    let calls = Calls::default();
    let expected = rendered(2, &options).into_iter().map(|item| item.file_name).collect::<Vec<_>>();
    assert!(expected.iter().all(|name| name.starts_with("set_") && !name.contains(['/', '\\'])));
    let result = execute(
        rendered(2, &options),
        &options,
        &Destination { dir: text(&root), exact: None },
        BatchAccess { sandboxed: true, resolve: &identity, directory_granted: &|path| path.starts_with(&root) },
        &calls,
    )
    .unwrap();
    assert_eq!(result.len(), 2);
    for ((path, bytes), name) in calls.writes.borrow().iter().zip(expected) {
        assert_eq!(Path::new(path), root.join("album").join(name));
        assert!(bytes.starts_with(b"\x89PNG\r\n\x1a\n"));
    }
}

#[test]
fn one_exact_file_save_does_not_ask_for_or_infer_a_parent_grant() {
    let options = png();
    let target = folder().join("selected.png");
    let calls = Calls::default();
    let destination = Destination { dir: text(&folder().join("unused")), exact: Some(text(&target)) };
    let result = execute(
        rendered(1, &options),
        &options,
        &destination,
        BatchAccess {
            sandboxed: true,
            resolve: &identity,
            directory_granted: &|_| panic!("one exact file without companions must retain the file-only Save flow"),
        },
        &calls,
    )
    .unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(calls.writes.borrow().len(), 1);
    assert_eq!(calls.writes.borrow()[0].0, text(&target));
}

#[test]
fn an_exact_path_cannot_hide_the_folder_needed_by_a_multi_photo_batch() {
    let options = png();
    let calls = Calls::default();
    let result = execute(
        rendered(2, &options),
        &options,
        &Destination { dir: text(&folder()), exact: Some(text(&folder().join("only-one.png"))) },
        BatchAccess { sandboxed: true, resolve: &identity, directory_granted: &|_| false },
        &calls,
    );
    assert!(result.unwrap_err().contains("folder picker"));
    calls.untouched();
}

fn original(reads: Arc<std::sync::atomic::AtomicUsize>) -> PreparedExport {
    PreparedExport {
        photo: lightcraft_catalog::PhotoId(1),
        file_name: "original.nef".into(),
        work: Work::File {
            path: text(&folder().join("source").join("original.nef")),
            read: Some(Arc::new(move |_| {
                reads.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                Ok(b"original source bytes".to_vec())
            })),
            packet: "<xmp/>".into(),
            dng: None,
            remove_location: false,
            label: "original.nef".into(),
            size: (4, 3),
        },
        guard: Arc::new(crate::originals::OriginalGuard::default()),
        weight: 1,
    }
}

#[test]
fn exact_original_and_its_xmp_are_preflighted_before_the_source_is_read() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let reads = Arc::new(AtomicUsize::new(0));
    let options = ExportOptions { format: ExportFormat::Original, ..Default::default() };
    let calls = Calls::default();
    let result = execute(
        vec![original(reads.clone())],
        &options,
        &Destination { dir: String::new(), exact: Some(text(&folder().join("selected.nef"))) },
        BatchAccess { sandboxed: true, resolve: &identity, directory_granted: &|_| false },
        &calls,
    );
    assert!(result.unwrap_err().contains("folder picker"));
    calls.untouched();
    assert_eq!(reads.load(Ordering::SeqCst), 0);
}

#[test]
fn a_separately_moved_xmp_destination_cannot_allow_a_partial_original_write() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let reads = Arc::new(AtomicUsize::new(0));
    let root = folder();
    let elsewhere = root.with_file_name("ungranted-sidecar");
    let resolve =
        |path: &Path| Ok(if path.extension().is_some_and(|ext| ext == "xmp") { elsewhere.join("selected.xmp") } else { path.to_path_buf() });
    let options = ExportOptions { format: ExportFormat::Original, ..Default::default() };
    let calls = Calls::default();
    let granted = |path: &Path| path.starts_with(&root);
    let result = execute(
        vec![original(reads.clone())],
        &options,
        &Destination { dir: String::new(), exact: Some(text(&root.join("selected.nef"))) },
        BatchAccess { sandboxed: true, resolve: &resolve, directory_granted: &granted },
        &calls,
    );
    assert!(result.unwrap_err().contains("ungranted-sidecar"));
    calls.untouched();
    assert_eq!(reads.load(Ordering::SeqCst), 0);
}

#[test]
fn captured_original_companion_plan_matches_the_actual_exported_packet() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let reads = Arc::new(AtomicUsize::new(0));
    let checked = RefCell::new(Vec::new());
    let resolve = |path: &Path| {
        checked.borrow_mut().push(path.to_path_buf());
        Ok(path.to_path_buf())
    };
    let target = folder().join("selected.nef");
    let options = ExportOptions { format: ExportFormat::Original, ..Default::default() };
    let output = with_batch_destination(
        vec![original(reads.clone())],
        &options,
        &Destination { dir: String::new(), exact: Some(text(&target)) },
        BatchAccess { sandboxed: true, resolve: &resolve, directory_granted: &|_| true },
        |mut items, _| items.remove(0).run(),
    )
    .unwrap();
    assert_eq!(reads.load(Ordering::SeqCst), 1);
    assert_eq!(output.sidecars, [("xmp", b"<xmp/>".to_vec())]);
    assert!(checked.borrow().contains(&target));
    assert!(checked.borrow().contains(&target.with_extension(output.sidecars[0].0)));
}

#[test]
fn a_moved_destination_is_resolved_before_derived_names_and_folder_checks() {
    let old = folder().join("old");
    let new = folder().join("current");
    let resolve = |path: &Path| Ok(path.strip_prefix(&old).map_or_else(|_| path.to_path_buf(), |relative| new.join(relative)));
    let granted = |path: &Path| path.starts_with(&new);
    let options = png();
    let calls = Calls::default();
    execute(
        rendered(2, &options),
        &options,
        &Destination { dir: text(&old), exact: None },
        BatchAccess { sandboxed: true, resolve: &resolve, directory_granted: &granted },
        &calls,
    )
    .unwrap();
    assert!(calls.writes.borrow().iter().all(|(path, _)| Path::new(path).starts_with(&new)));
}

#[test]
fn a_late_resolution_failure_precedes_every_batch_side_effect() {
    let options = png();
    let items = rendered(2, &options);
    let denied = items[1].file_name.clone();
    let resolve = |path: &Path| {
        if path.file_name().is_some_and(|name| name == denied.as_str()) { Err("stored bookmark denied".into()) } else { Ok(path.to_path_buf()) }
    };
    let calls = Calls::default();
    let result = execute(
        items,
        &options,
        &Destination { dir: text(&folder()), exact: None },
        BatchAccess { sandboxed: true, resolve: &resolve, directory_granted: &|_| true },
        &calls,
    );
    assert_eq!(result.unwrap_err(), "stored bookmark denied");
    calls.untouched();
}

#[test]
fn traversal_in_a_derived_subfolder_cannot_escape_an_explicit_grant() {
    let options = ExportOptions { subfolder: "../outside".into(), ..png() };
    let calls = Calls::default();
    let result = execute(
        rendered(1, &options),
        &options,
        &Destination { dir: text(&folder()), exact: None },
        BatchAccess { sandboxed: true, resolve: &identity, directory_granted: &|_| true },
        &calls,
    );
    assert!(result.unwrap_err().contains("folder picker"));
    calls.untouched();
}

#[test]
fn ordinary_hosts_keep_relative_derived_paths_without_new_scope_checks() {
    let options = png();
    let calls = Calls::default();
    let result = execute(
        rendered(2, &options),
        &options,
        &Destination { dir: "ordinary-output".into(), exact: None },
        BatchAccess {
            sandboxed: false,
            resolve: &identity,
            directory_granted: &|_| panic!("ordinary host must not acquire a sandbox-only directory restriction"),
        },
        &calls,
    );
    if crate::macos_mas::enabled() {
        // Injecting an ordinary front-door policy cannot bypass the real MAS Placer guard.
        assert!(result.unwrap_err().contains("invalid file path"));
        assert_eq!(calls.entered.get(), 1);
        assert_eq!(calls.progress.get(), 1);
        assert_eq!(calls.collisions.get(), 1);
        assert!(calls.writes.borrow().is_empty());
    } else {
        result.unwrap();
        assert_eq!(calls.writes.borrow().len(), 2);
        assert!(calls.writes.borrow().iter().all(|(path, _)| Path::new(path).starts_with("ordinary-output")));
    }
}

#[test]
fn public_batch_uses_the_real_mas_registry_and_keeps_ordinary_exports() {
    let options = png();
    let calls = Calls::default();
    let mut write = |path: &str, bytes: &[u8]| {
        calls.writes.borrow_mut().push((path.into(), bytes.to_vec()));
        Ok(())
    };
    let exists = |_: &str| {
        calls.collisions.set(calls.collisions.get() + 1);
        false
    };
    let mut progress = |_: usize, _: &str| {
        calls.progress.set(calls.progress.get() + 1);
        true
    };
    let result =
        run_batch(rendered(2, &options), &options, &Destination { dir: text(&folder()), exact: None }, &mut write, &exists, true, &mut progress);
    if crate::macos_mas::enabled() {
        assert!(result.unwrap_err().contains("folder picker"));
        calls.untouched();
    } else {
        assert_eq!(result.unwrap().len(), 2);
        assert_eq!(calls.writes.borrow().len(), 2);
    }
}
