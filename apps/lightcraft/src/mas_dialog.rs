//! Native panels retain precisely the files and folders the user chose.
use std::path::{Path, PathBuf};

use crate::dialog_filter::FileDialogExt as _;
use lightcraft_engine::macos_mas;

pub struct Dialog(pub rfd::FileDialog);

fn synchronous_allowed() -> bool {
    if macos_mas::enabled() {
        log::error!("App Store native file selection must use the nonblocking picker");
        false
    } else {
        true
    }
}

impl Dialog {
    pub fn set_title(self, title: impl Into<String>) -> Self {
        Self(self.0.set_title(title))
    }

    pub fn set_file_name(self, name: impl Into<String>) -> Self {
        Self(self.0.set_file_name(name))
    }

    pub fn set_directory(self, path: impl AsRef<Path>) -> Self {
        Self(self.0.set_directory(path))
    }

    pub fn add_filter_nocase(self, name: impl Into<String>, extensions: &[&str]) -> Self {
        Self(self.0.add_filter_nocase(name, extensions))
    }

    pub fn pick_file(self) -> Option<PathBuf> {
        if !synchronous_allowed() {
            return None;
        }
        self.0.pick_file().and_then(|path| macos_mas::picked(path, false))
    }

    pub fn pick_folder(self) -> Option<PathBuf> {
        if !synchronous_allowed() {
            return None;
        }
        self.0.pick_folder().and_then(|path| macos_mas::picked(path, false))
    }

    pub fn pick_files(self) -> Option<Vec<PathBuf>> {
        if !synchronous_allowed() {
            return None;
        }
        self.0.pick_files().map(|paths| paths.into_iter().filter_map(|path| macos_mas::picked(path, false)).collect())
    }

    pub fn save_file(self) -> Option<PathBuf> {
        if !synchronous_allowed() {
            return None;
        }
        self.0.save_file().and_then(|path| macos_mas::picked(path, true))
    }
}

/// Begin a parented AppKit sheet on the main thread, then wait only on a worker.
/// rfd's synchronous API invokes runModal even when called from a worker; an
/// async future must be constructed here while its real parent window is alive.
#[cfg(all(target_os = "macos", feature = "mac-app-store"))]
pub fn start_sheet(
    request: lightcraft_ui_egui::pick::PickRequest,
    parent: Option<&rfd::AsyncFileDialog>,
    repaint: egui::Context,
) -> Result<std::sync::mpsc::Receiver<Vec<String>>, String> {
    use lightcraft_ui_egui::pick::PickKind;
    use std::future::Future;
    use std::pin::Pin;
    type Answer = Pin<Box<dyn Future<Output = Vec<String>> + Send>>;

    let mtm = objc2::MainThreadMarker::new().ok_or("File selection must start from the app window")?;
    if !objc2_app_kit::NSApplication::sharedApplication(mtm).isRunning() {
        return Err("The app window is not ready for file selection".into());
    }
    let mut dialog = parent.cloned().ok_or("No app window is available for file selection")?;
    if let Some(title) = request.title {
        dialog = dialog.set_title(title);
    }
    if let Some((name, extensions)) = request.filter {
        dialog = dialog.add_filter(name, &crate::dialog_filter::dialog_extensions(extensions));
    }
    if let Some(name) = request.file_name {
        dialog = dialog.set_file_name(name);
    }

    // Start the waiter first: a sheet is never left open without a receiver.
    let (future_tx, future_rx) = std::sync::mpsc::channel::<Answer>();
    let (answer_tx, answer_rx) = std::sync::mpsc::channel();
    std::thread::Builder::new()
        .name("file-dialog-sheet".into())
        .spawn(move || {
            if let Ok(answer) = future_rx.recv() {
                let _ = answer_tx.send(pollster::block_on(answer));
                repaint.request_repaint();
            }
        })
        .map_err(|error| format!("Could not start the file chooser: {error}"))?;
    let save = request.kind == PickKind::Save;
    let one = move |file: Option<rfd::FileHandle>| {
        file.into_iter().filter_map(|file| macos_mas::picked(file.path(), save)).map(|path| path.to_string_lossy().into_owned()).collect()
    };
    // These method calls construct ModalFuture and begin the sheet immediately
    // on main; only awaiting their completion is moved to the worker.
    let answer: Answer = match request.kind {
        PickKind::File => {
            let answer = dialog.pick_file();
            Box::pin(async move { one(answer.await) })
        }
        PickKind::Files => {
            let answer = dialog.pick_files();
            Box::pin(async move {
                answer
                    .await
                    .unwrap_or_default()
                    .into_iter()
                    .filter_map(|file| macos_mas::picked(file.path(), false))
                    .map(|path| path.to_string_lossy().into_owned())
                    .collect()
            })
        }
        PickKind::Folder => {
            let answer = dialog.pick_folder();
            Box::pin(async move { one(answer.await) })
        }
        PickKind::Save => {
            let answer = dialog.save_file();
            Box::pin(async move { one(answer.await) })
        }
    };
    future_tx.send(answer).map_err(|_| "The file chooser stopped before it could answer".to_owned())?;
    Ok(answer_rx)
}
