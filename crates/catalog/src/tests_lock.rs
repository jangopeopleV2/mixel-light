//! Issue #99: one process per library.

use std::path::PathBuf;

use crate::lock::{LOCK, OWNER};
use crate::*;

/// Taken by every test here that holds a lock in this process. On Unix, spawning the child of
/// [`lock_held_by_another_process_is_released_when_it_dies`] briefly copies this process's file
/// descriptors, close-on-exec lock files included, until the child's exec closes them. A lock
/// another test drops in that moment stays held by the copy, so acquiring it again could fail with
/// "in use".
static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn serial() -> std::sync::MutexGuard<'static, ()> {
    SERIAL.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn temp_dir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("lc-lock-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn second_opener_is_refused_until_the_first_lets_go() {
    let _serial = serial();
    let dir = temp_dir("twice");
    let first = LibraryLock::acquire(&dir, "MIXEL Light").unwrap();
    assert!(first.held());
    let owner: LockOwner = serde_json::from_slice(&std::fs::read(dir.join(OWNER)).unwrap()).unwrap();
    assert_eq!((owner.pid, owner.program.as_str()), (std::process::id(), "MIXEL Light"));

    let e = LibraryLock::acquire(&dir, "lightcraft-cli").unwrap_err();
    let LockError::InUse(Some(who)) = &e else { panic!("{e:?}") };
    assert_eq!(who.pid, std::process::id());
    let msg = e.to_string();
    assert!(msg.contains("already open in MIXEL Light") && msg.contains(&format!("process {}", std::process::id())), "{msg}");

    drop(first);
    assert!(!dir.join(OWNER).exists(), "the owner note goes with the lock");
    assert!(dir.join(LOCK).exists(), "the lock file itself may stay: only a held lock refuses");
    let again = LibraryLock::acquire(&dir, "lightcraft-cli").unwrap();
    assert!(again.held());
    drop(again);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Files left behind by a crash (lock file + owner note) don't lock anyone out.
#[test]
fn leftover_lock_files_are_not_a_lock() {
    let _serial = serial();
    let dir = temp_dir("stale");
    std::fs::write(dir.join(LOCK), b"").unwrap();
    std::fs::write(dir.join(OWNER), br#"{"pid":999999,"host":"elsewhere","program":"MIXEL Light","version":"0.2.0","since":1}"#).unwrap();
    let l = LibraryLock::acquire(&dir, "MIXEL Light").unwrap();
    assert!(l.held());
    let owner: LockOwner = serde_json::from_slice(&std::fs::read(dir.join(OWNER)).unwrap()).unwrap();
    assert_eq!(owner.pid, std::process::id(), "the note now names this process");
    drop(l);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Another process holds the lock; when it is killed (a crash), the OS releases it.
#[test]
fn lock_held_by_another_process_is_released_when_it_dies() {
    let _serial = serial();
    let dir = temp_dir("child");
    let exe = std::env::current_exe().unwrap();
    let mut child = std::process::Command::new(exe)
        .args(["--exact", "tests_lock::lock_holder_child", "--ignored", "--test-threads=1"])
        .env("LC_LOCK_CHILD_DIR", &dir)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
    while !dir.join("child-locked").exists() {
        assert!(child.try_wait().unwrap().is_none(), "child exited before locking");
        assert!(std::time::Instant::now() < deadline, "child never locked");
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    let e = LibraryLock::acquire(&dir, "MIXEL Light").unwrap_err();
    let LockError::InUse(Some(who)) = &e else { panic!("{e:?}") };
    assert_eq!(who.pid, child.id());
    assert!(e.to_string().contains("on this computer") || who.host.is_empty(), "{e}");

    child.kill().unwrap(); // SIGKILL: no clean-up runs, like a crash
    child.wait().unwrap();
    assert!(dir.join(OWNER).exists(), "the crashed holder's note is left behind");
    let l = LibraryLock::acquire(&dir, "MIXEL Light").expect("the OS released the dead process's lock");
    assert!(l.held());
    drop(l);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Run by [`lock_held_by_another_process_is_released_when_it_dies`] in a child process: take the
/// lock, say so, and wait to be killed.
#[test]
#[ignore = "helper process for lock_held_by_another_process_is_released_when_it_dies"]
fn lock_holder_child() {
    let Some(dir) = std::env::var_os("LC_LOCK_CHILD_DIR") else { return };
    let _l = LibraryLock::acquire(std::path::Path::new(&dir), "lightcraft-cli").unwrap();
    std::fs::write(std::path::Path::new(&dir).join("child-locked"), b"").unwrap();
    std::thread::sleep(std::time::Duration::from_secs(120));
}

/// Issue #680: a process spawned by any thread holds a copy of every descriptor until it execs
/// (close-on-exec only closes them then), and `flock` lasts while any copy of the descriptor is
/// open. Dropping the `LibraryLock` must still free the library, so it releases explicitly rather
/// than relying on its handle closing. A `try_clone` of the descriptor stands in for the child's
/// copy: it refers to the same open file description, which is exactly what the child gets.
#[cfg(unix)]
#[test]
fn the_library_is_free_once_dropped_even_if_a_child_holds_a_copy() {
    let dir = temp_dir("copy");
    let first = LibraryLock::acquire(&dir, "MIXEL Light").unwrap();
    let copy = first.descriptor_copy().unwrap();
    drop(first);
    let again = LibraryLock::acquire(&dir, "lightcraft-cli").expect("the library is free once its LibraryLock is dropped");
    assert!(again.held());
    drop((again, copy));
    let _ = std::fs::remove_dir_all(&dir);
}

/// Issue #680, as it happens in practice: one thread spawns cheap children while another acquires,
/// drops and re-acquires the lock on its own directory. Every acquire must succeed (before the fix
/// a few per cent were refused with `InUse(None)`). The two-second stress starts only after a
/// successful child; startup, each child's lifetime and worker cleanup also have deadlines.
#[cfg(unix)]
#[test]
fn dropping_the_lock_frees_the_library_while_another_thread_spawns_processes() {
    use std::process::{Child, Command, Stdio};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, mpsc};
    use std::time::{Duration, Instant};

    fn finish_child(child: &mut Child) -> std::result::Result<(), String> {
        let deadline = Instant::now() + Duration::from_secs(5);
        let failure = loop {
            match child.try_wait() {
                Ok(Some(status)) => return if status.success() { Ok(()) } else { Err(format!("true child {} exited with {status}", child.id())) },
                Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(1)),
                Ok(None) => break format!("true child {} exceeded its five-second lifetime", child.id()),
                Err(e) => break format!("waiting for true child {}: {e}", child.id()),
            }
        };
        // Kill only our child, then poll for a reaped exit rather than an unbounded wait.
        let killed = child.kill();
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            match child.try_wait() {
                Ok(Some(status)) => return Err(format!("{failure}; kill: {killed:?}; reaped: {status}")),
                Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(1)),
                Ok(None) => return Err(format!("{failure}; kill: {killed:?}; child was not reaped within two seconds")),
                Err(e) => return Err(format!("{failure}; kill: {killed:?}; reaping child: {e}")),
            }
        }
    }

    let dir = temp_dir("spawn");
    let stop = Arc::new(AtomicBool::new(false));
    let cycling = Arc::new(AtomicBool::new(false));
    let (ready_tx, ready_rx) = mpsc::channel();
    let (done_tx, done_rx) = mpsc::channel();
    let spawner = {
        let stop = Arc::clone(&stop);
        let cycling = Arc::clone(&cycling);
        std::thread::spawn(move || {
            let result = (|| -> std::result::Result<(usize, usize), String> {
                let (mut spawned, mut overlapped) = (0usize, 0usize);
                while !stop.load(Ordering::Acquire) {
                    let during_cycles = cycling.load(Ordering::Acquire);
                    // Direct executable lookup is portable: macOS has /usr/bin/true, not /bin/true.
                    let mut child =
                        Command::new("true").stdout(Stdio::null()).stderr(Stdio::null()).spawn().map_err(|e| format!("spawning true: {e}"))?;
                    finish_child(&mut child)?;
                    spawned += 1;
                    if during_cycles && cycling.load(Ordering::Acquire) {
                        overlapped += 1;
                    }
                    if spawned == 1 {
                        ready_tx.send(()).map_err(|e| format!("reporting the first successful true child: {e}"))?;
                    }
                }
                Ok((spawned, overlapped))
            })();
            let _ = done_tx.send(result);
        })
    };
    let started = ready_rx.recv_timeout(Duration::from_secs(30));
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut cycles = 0usize;
    let mut refused = Vec::new();
    if started.is_ok() {
        cycling.store(true, Ordering::Release);
        while cycles < 200_000 && Instant::now() < deadline {
            match LibraryLock::acquire(&dir, "MIXEL Light") {
                Ok(l) if l.held() => {} // dropped here: the next cycle must get it back
                Ok(_) => refused.push(format!("cycle {cycles}: acquired without holding the lock")),
                Err(e) => refused.push(format!("cycle {cycles}: {e:?}")),
            }
            cycles += 1;
        }
    }
    cycling.store(false, Ordering::Release);
    stop.store(true, Ordering::Release);
    let finished = done_rx.recv_timeout(Duration::from_secs(10));
    // A reported result means all children were reaped (or their cleanup failure was reported).
    // Don't block forever joining a worker that missed its cleanup deadline.
    if !matches!(finished, Err(mpsc::RecvTimeoutError::Timeout)) {
        spawner.join().expect("the spawning thread panicked");
    }
    let _ = std::fs::remove_dir_all(&dir);
    let (spawned, overlapped) = finished.expect("the spawning thread did not finish within ten seconds").expect("spawning/reaping true failed");
    assert!(started.is_ok(), "the spawning thread did not start within thirty seconds: {started:?}");
    assert!(spawned > 0, "the spawning thread never got a child off the ground");
    assert!(cycles > 0 && overlapped > 0, "no successful child lifetime overlapped the {cycles} lock cycles ({spawned} children total)");
    assert!(
        refused.is_empty(),
        "{} of {cycles} re-acquires refused while {spawned} children were spawned: {:?}",
        refused.len(),
        &refused[..refused.len().min(3)]
    );
}
