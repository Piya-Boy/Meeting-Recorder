//! Downloading the model files, resumable and checked.
//!
//! Each file goes to `<name>.part` and is renamed only after its SHA-256 matches, so a file
//! under its real name is always a verified one. A download cut short (connection lost, app
//! closed, cancel) carries on from the bytes already saved. Follows PoodPim's `src/model.rs`
//! (commit 15e07cc, MIT, Copyright (c) 2026 Thirawat Sutalungka,
//! https://github.com/thirawat/PoodPim; see THIRD_PARTY_NOTICES.md).

use std::cell::Cell;
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use crate::asr::model::{self, Precision};

/// Tries per file. A try that saved some bytes does not count against this.
const ATTEMPTS: u32 = 6;
/// No bytes for this long: the connection is stuck, start a new one (resuming).
const STALL: Duration = Duration::from_secs(45);

/// A file to download and what it must be.
struct FileSpec {
    name: String,
    size: u64,
    sha256: String,
}

/// Timing, replaceable so tests do not wait.
struct Tuning {
    retry_wait: fn(u32) -> Duration,
    stall: Duration,
}

fn real_retry_wait(attempt: u32) -> Duration {
    Duration::from_secs(2u64 << attempt.min(4)) // 4, 8, 16, 32, 32 s
}

enum FetchError {
    Cancelled,
    Failed(String),
}

const CANCELLED: &str = "cancelled";

/// Downloads the model of `p` into `dir` from `base_url` (the pinned revision in the app),
/// checks every file, unpacks the archive and marks the folder ready. `progress(done, total)`
/// counts bytes over all files and never goes back. Returns `Err("cancelled")` when `cancel`
/// is set; the bytes already saved stay for the next run.
pub fn download(
    dir: &Path,
    p: Precision,
    base_url: &str,
    progress: &dyn Fn(u64, u64),
    cancel: &AtomicBool,
) -> Result<(), String> {
    let files: Vec<FileSpec> = model::manifest(p)
        .iter()
        .map(|f| FileSpec {
            name: f.name.to_owned(),
            size: f.size,
            sha256: f.sha256.to_owned(),
        })
        .collect();
    let zip = files[1].name.clone();
    let tuning = Tuning {
        retry_wait: real_retry_wait,
        stall: STALL,
    };
    download_files(dir, &files, &zip, base_url, progress, cancel, &tuning)
}

/// Whether `dir` already holds a finished install of `files` (the archive is gone by then).
fn installed(dir: &Path, files: &[FileSpec], zip_name: &str) -> bool {
    dir.join(model::READY_MARKER).is_file()
        && files
            .iter()
            .filter(|f| f.name != zip_name)
            .all(|f| fs::metadata(dir.join(&f.name)).is_ok_and(|m| m.len() == f.size))
        && model::decoder_path(dir).is_file()
        && model::tokens_path(dir).is_file()
}

fn download_files(
    dir: &Path,
    files: &[FileSpec],
    zip_name: &str,
    base_url: &str,
    progress: &dyn Fn(u64, u64),
    cancel: &AtomicBool,
    tuning: &Tuning,
) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    let total: u64 = files.iter().map(|f| f.size).sum();
    if installed(dir, files, zip_name) {
        progress(total, total);
        return Ok(());
    }
    // The bar never goes back, even when a file has to start again from zero.
    let shown = Cell::new(0u64);
    let report = |n: u64| {
        let v = n.max(shown.get());
        shown.set(v);
        progress(v, total);
    };
    let base = base_url.trim_end_matches('/');
    let mut done = 0u64;
    for f in files {
        let dest = dir.join(&f.name);
        let complete = fs::metadata(&dest).is_ok_and(|m| m.len() == f.size);
        if !complete {
            fetch(base, f, &dest, &|n| report(done + n), cancel, tuning)?;
        }
        done += f.size;
        report(done);
    }
    let zip_path = dir.join(zip_name);
    model::extract_zip(&zip_path, dir)?;
    fs::remove_file(&zip_path).map_err(|e| format!("cannot remove {zip_name}: {e}"))?;
    model::mark_ready(dir, &files[0].name).map_err(|e| format!("cannot finish the install: {e}"))
}

fn part_path(dest: &Path) -> PathBuf {
    let mut name = dest.file_name().unwrap_or_default().to_os_string();
    name.push(".part");
    dest.with_file_name(name)
}

/// Sleeps `d` in short slices; `false` when cancelled meanwhile.
fn wait(d: Duration, cancel: &AtomicBool) -> bool {
    let end = std::time::Instant::now() + d;
    while std::time::Instant::now() < end {
        if cancel.load(Ordering::Relaxed) {
            return false;
        }
        std::thread::sleep(Duration::from_millis(50).min(d));
    }
    !cancel.load(Ordering::Relaxed)
}

/// Downloads one file through `<dest>.part`, retrying and resuming, then checks and renames.
fn fetch(
    base: &str,
    f: &FileSpec,
    dest: &Path,
    progress: &dyn Fn(u64),
    cancel: &AtomicBool,
    tuning: &Tuning,
) -> Result<(), String> {
    let part = part_path(dest);
    let saved = || fs::metadata(&part).map(|m| m.len()).unwrap_or(0);
    let mut last_err = String::new();
    let (mut attempt, mut damaged) = (0u32, 0u32);
    while attempt < ATTEMPTS && damaged < 2 {
        if cancel.load(Ordering::Relaxed) {
            return Err(CANCELLED.to_owned());
        }
        let before = saved();
        if attempt > 0 && !wait((tuning.retry_wait)(attempt), cancel) {
            return Err(CANCELLED.to_owned());
        }
        match fetch_once(base, f, &part, progress, cancel, tuning) {
            Ok(()) => match model::sha256_file(&part, cancel) {
                Ok(Some(got)) if got == f.sha256 => {
                    return fs::rename(&part, dest)
                        .map_err(|e| format!("cannot save {}: {e}", f.name));
                }
                Ok(Some(got)) => {
                    // Once more from the start; a second bad copy is an error.
                    let _ = fs::remove_file(&part);
                    damaged += 1;
                    attempt += 1;
                    last_err = format!(
                        "{} is damaged (SHA-256 {got}, expected {})",
                        f.name, f.sha256
                    );
                }
                Ok(None) => return Err(CANCELLED.to_owned()),
                Err(e) => return Err(format!("cannot check {}: {e}", f.name)),
            },
            Err(FetchError::Cancelled) => return Err(CANCELLED.to_owned()),
            Err(FetchError::Failed(e)) => {
                last_err = e;
                // A try that got some of the file does not count: a connection that keeps
                // dropping still gets there.
                attempt = if saved() > before { 1 } else { attempt + 1 };
            }
        }
    }
    Err(format!("downloading {} failed: {last_err}", f.name))
}

/// HTTP client using the OS TLS stack (SChannel), so no C toolchain is needed to build.
fn agent(stall: Duration) -> ureq::Agent {
    use ureq::tls::{TlsConfig, TlsProvider};
    ureq::Agent::config_builder()
        .tls_config(
            TlsConfig::builder()
                .provider(TlsProvider::NativeTls)
                .build(),
        )
        .timeout_resolve(Some(Duration::from_secs(30)))
        .timeout_connect(Some(Duration::from_secs(30)))
        .timeout_per_read(Some(stall))
        .build()
        .new_agent()
}

/// One request: asks for the bytes after those already in `part` and appends them.
fn fetch_once(
    base: &str,
    f: &FileSpec,
    part: &Path,
    progress: &dyn Fn(u64),
    cancel: &AtomicBool,
    tuning: &Tuning,
) -> Result<(), FetchError> {
    let fail = |e: &dyn std::fmt::Display| FetchError::Failed(format!("{}: {e}", f.name));
    let mut have = fs::metadata(part).map(|m| m.len()).unwrap_or(0);
    if have > f.size {
        fs::remove_file(part).map_err(|e| fail(&e))?;
        have = 0;
    }
    if have == f.size {
        progress(have);
        return Ok(());
    }
    let mut req = agent(tuning.stall).get(&format!("{base}/{}", f.name));
    if have > 0 {
        req = req.header("Range", format!("bytes={have}-"));
    }
    let resp = req.call().map_err(|e| fail(&e))?;
    // 206 from where we asked = the rest of the file; anything else = all of it.
    let resumed = resp.status().as_u16() == 206
        && resp
            .headers()
            .get("content-range")
            .and_then(|v| v.to_str().ok())
            .is_some_and(|v| v.starts_with(&format!("bytes {have}-")));
    if !resumed {
        have = 0;
    }
    let mut out = OpenOptions::new()
        .create(true)
        .write(true)
        .append(resumed)
        .truncate(!resumed)
        .open(part)
        .map_err(|e| fail(&e))?;
    let mut reader = resp.into_body().into_with_config().limit(f.size).reader();
    let mut buf = vec![0u8; 1 << 20];
    let mut n = have;
    progress(n);
    loop {
        if cancel.load(Ordering::Relaxed) {
            let _ = out.flush();
            return Err(FetchError::Cancelled);
        }
        let k = reader.read(&mut buf).map_err(|e| fail(&e))?;
        if k == 0 {
            break;
        }
        out.write_all(&buf[..k]).map_err(|e| fail(&e))?;
        n += k as u64;
        progress(n);
    }
    out.flush().map_err(|e| fail(&e))?;
    if n != f.size {
        return Err(fail(&format!("got {n} of {} bytes", f.size)));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::net::{TcpListener, TcpStream};
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::{Arc, Mutex};

    #[derive(Clone, Default)]
    struct Behavior {
        /// Answer `200` with the whole body even when a range is asked for.
        ignore_range: bool,
        /// The first this many requests are cut after `drop_after` body bytes.
        drop_first: u32,
        drop_after: usize,
        /// Close every connection without answering.
        always_drop: bool,
        /// Flip a byte in the body of the data file.
        corrupt: bool,
        /// Send only this many bytes of the data file the first time it is asked for from the
        /// start, then hold the connection open until the client hangs up (0 = off). Lets a test
        /// cancel with part of the file missing whatever size the client reads in.
        stall_after: usize,
    }

    type RequestLog = Arc<Mutex<Vec<(String, Option<u64>)>>>;

    struct Server {
        base: String,
        /// `(path, start of the Range header)` of every request.
        log: RequestLog,
        stop: Arc<AtomicBool>,
        port: u16,
    }

    impl Drop for Server {
        fn drop(&mut self) {
            self.stop.store(true, Ordering::SeqCst);
            let _ = TcpStream::connect(("127.0.0.1", self.port));
        }
    }

    fn handle(
        mut s: TcpStream,
        files: &HashMap<String, Vec<u8>>,
        b: &Behavior,
        n: u32,
        log: &Mutex<Vec<(String, Option<u64>)>>,
    ) {
        let mut head = Vec::new();
        let mut buf = [0u8; 1024];
        while !head.windows(4).any(|w| w == b"\r\n\r\n") {
            match s.read(&mut buf) {
                Ok(0) | Err(_) => return,
                Ok(k) => head.extend_from_slice(&buf[..k]),
            }
        }
        let text = String::from_utf8_lossy(&head).into_owned();
        let path = text
            .lines()
            .next()
            .and_then(|l| l.split_whitespace().nth(1))
            .unwrap_or("/")
            .to_owned();
        let range = text.lines().find_map(|l| {
            let l = l.to_ascii_lowercase();
            l.strip_prefix("range: bytes=")
                .and_then(|r| r.trim_end_matches('-').trim().parse::<u64>().ok())
        });
        log.lock().unwrap().push((path.clone(), range));
        if b.always_drop {
            return;
        }
        let Some(full) = files.get(path.trim_start_matches('/')) else {
            let _ = s.write_all(
                b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
            );
            return;
        };
        let mut body = full.clone();
        if b.corrupt && path.ends_with(".data") {
            body[10] ^= 0xff;
        }
        let (status, start) = match range {
            Some(r) if !b.ignore_range => ("206 Partial Content", r as usize),
            _ => ("200 OK", 0),
        };
        let mut head = format!(
            "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n",
            body.len() - start
        );
        if start > 0 {
            head.push_str(&format!(
                "Content-Range: bytes {start}-{}/{}\r\n",
                body.len() - 1,
                body.len()
            ));
        }
        head.push_str("\r\n");
        let _ = s.write_all(head.as_bytes());
        let rest = &body[start..];
        if b.stall_after > 0 && range.is_none() && path.ends_with(".data") {
            let _ = s.write_all(&rest[..b.stall_after.min(rest.len())]);
            s.set_read_timeout(Some(Duration::from_millis(20))).ok();
            let began = std::time::Instant::now();
            let mut one = [0u8; 1];
            while began.elapsed() < Duration::from_secs(10) {
                match s.read(&mut one) {
                    Ok(0) => break,
                    Err(e)
                        if matches!(
                            e.kind(),
                            std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                        ) => {}
                    _ => break,
                }
            }
            return;
        }
        if n < b.drop_first {
            let _ = s.write_all(&rest[..b.drop_after.min(rest.len())]);
            return; // dropped: the connection closes with the body unfinished
        }
        let _ = s.write_all(rest);
    }

    fn serve(files: HashMap<String, Vec<u8>>, b: Behavior) -> Server {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let log = Arc::new(Mutex::new(Vec::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let (log2, stop2) = (log.clone(), stop.clone());
        let counter = AtomicU32::new(0);
        std::thread::spawn(move || {
            for conn in listener.incoming() {
                if stop2.load(Ordering::SeqCst) {
                    break;
                }
                let Ok(conn) = conn else { continue };
                let n = counter.fetch_add(1, Ordering::SeqCst);
                handle(conn, &files, &b, n, &log2);
            }
        });
        Server {
            base: format!("http://127.0.0.1:{port}"),
            log,
            stop,
            port,
        }
    }

    fn sha_hex(bytes: &[u8]) -> String {
        use sha2::{Digest, Sha256};
        Sha256::digest(bytes)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect()
    }

    fn data_bytes() -> Vec<u8> {
        (0..100_000u32)
            .map(|i| (i.wrapping_mul(2_654_435_761) >> 24) as u8)
            .collect()
    }

    fn zip_bytes() -> Vec<u8> {
        let dir = crate::audio::temp_dir("download-zip");
        let path = dir.join("m.zip");
        let file = std::fs::File::create(&path).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        let opts = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        for (name, body) in [
            ("encoder.int8.onnx", &b"enc"[..]),
            ("decoder_joint.onnx", &b"dec"[..]),
            ("tokens.txt", &b"tok"[..]),
        ] {
            zip.start_file(name, opts).unwrap();
            zip.write_all(body).unwrap();
        }
        zip.finish().unwrap();
        std::fs::read(path).unwrap()
    }

    /// The fake model: its files, what the server holds, and the specs the client checks.
    fn fake_model() -> (HashMap<String, Vec<u8>>, Vec<FileSpec>) {
        let parts = [
            ("encoder.int8.onnx.data", data_bytes()),
            ("model.zip", zip_bytes()),
            ("LICENSE", b"licence text".to_vec()),
        ];
        let specs = parts
            .iter()
            .map(|(n, b)| FileSpec {
                name: (*n).to_owned(),
                size: b.len() as u64,
                sha256: sha_hex(b),
            })
            .collect();
        let files = parts.into_iter().map(|(n, b)| (n.to_owned(), b)).collect();
        (files, specs)
    }

    fn tuning() -> Tuning {
        Tuning {
            retry_wait: |_| Duration::from_millis(1),
            stall: Duration::from_secs(5),
        }
    }

    fn run(
        dir: &Path,
        specs: &[FileSpec],
        server: &Server,
        cancel: &AtomicBool,
        progress: &dyn Fn(u64, u64),
    ) -> Result<(), String> {
        download_files(
            dir,
            specs,
            "model.zip",
            &server.base,
            progress,
            cancel,
            &tuning(),
        )
    }

    fn no_progress(_: u64, _: u64) {}

    fn leftovers(dir: &Path) -> Vec<String> {
        std::fs::read_dir(dir)
            .unwrap()
            .filter_map(Result::ok)
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n.ends_with(".part"))
            .collect()
    }

    #[test]
    fn full_download_installs_and_reports_monotonic_progress() {
        let (files, specs) = fake_model();
        let server = serve(files, Behavior::default());
        let dir = crate::audio::temp_dir("download-full").join("m");
        let seen = Mutex::new(Vec::new());
        run(&dir, &specs, &server, &AtomicBool::new(false), &|d, t| {
            seen.lock().unwrap().push((d, t));
        })
        .unwrap();

        assert_eq!(
            std::fs::read(dir.join("encoder.int8.onnx.data")).unwrap(),
            data_bytes()
        );
        assert!(dir.join("LICENSE").is_file());
        assert!(dir.join("encoder.int8.onnx").is_file());
        assert!(dir.join("decoder_joint.onnx").is_file());
        assert!(dir.join("tokens.txt").is_file());
        assert!(!dir.join("model.zip").exists(), "the archive is removed");
        assert!(dir.join(crate::asr::model::READY_MARKER).is_file());
        assert!(leftovers(&dir).is_empty());

        let seen = seen.into_inner().unwrap();
        let total: u64 = specs.iter().map(|s| s.size).sum();
        assert!(seen.iter().all(|(_, t)| *t == total));
        assert!(
            seen.windows(2).all(|w| w[0].0 <= w[1].0),
            "progress went back: {seen:?}"
        );
        assert_eq!(seen.last().unwrap().0, total);
    }

    #[test]
    fn interrupted_file_resumes_with_a_range_request() {
        let (files, specs) = fake_model();
        let server = serve(files, Behavior::default());
        let dir = crate::audio::temp_dir("download-resume").join("m");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("encoder.int8.onnx.data.part"),
            &data_bytes()[..40_000],
        )
        .unwrap();
        run(&dir, &specs, &server, &AtomicBool::new(false), &no_progress).unwrap();
        assert_eq!(
            std::fs::read(dir.join("encoder.int8.onnx.data")).unwrap(),
            data_bytes()
        );
        let log = server.log.lock().unwrap();
        assert!(
            log.contains(&("/encoder.int8.onnx.data".to_owned(), Some(40_000))),
            "{log:?}"
        );
    }

    #[test]
    fn server_that_ignores_range_restarts_the_file() {
        let (files, specs) = fake_model();
        let server = serve(
            files,
            Behavior {
                ignore_range: true,
                ..Behavior::default()
            },
        );
        let dir = crate::audio::temp_dir("download-norange").join("m");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("encoder.int8.onnx.data.part"),
            &data_bytes()[..40_000],
        )
        .unwrap();
        let seen = Mutex::new(Vec::new());
        run(&dir, &specs, &server, &AtomicBool::new(false), &|d, _| {
            seen.lock().unwrap().push(d);
        })
        .unwrap();
        assert_eq!(
            std::fs::read(dir.join("encoder.int8.onnx.data")).unwrap(),
            data_bytes()
        );
        let seen = seen.into_inner().unwrap();
        assert!(seen.windows(2).all(|w| w[0] <= w[1]), "progress went back");
    }

    #[test]
    fn corrupted_download_fails_and_leaves_no_file() {
        let (files, specs) = fake_model();
        let server = serve(
            files,
            Behavior {
                corrupt: true,
                ..Behavior::default()
            },
        );
        let dir = crate::audio::temp_dir("download-corrupt").join("m");
        let err = run(&dir, &specs, &server, &AtomicBool::new(false), &no_progress).unwrap_err();
        assert!(err.contains("encoder.int8.onnx.data"), "{err}");
        assert!(err.contains("damaged"), "{err}");
        assert!(!dir.join("encoder.int8.onnx.data").exists());
        assert!(!dir.join("encoder.int8.onnx.data.part").exists());
        assert!(!dir.join(crate::asr::model::READY_MARKER).exists());
    }

    #[test]
    fn dropped_connections_are_retried() {
        let (files, specs) = fake_model();
        let server = serve(
            files,
            Behavior {
                drop_first: 2,
                drop_after: 30_000,
                ..Behavior::default()
            },
        );
        let dir = crate::audio::temp_dir("download-drop").join("m");
        run(&dir, &specs, &server, &AtomicBool::new(false), &no_progress).unwrap();
        assert_eq!(
            std::fs::read(dir.join("encoder.int8.onnx.data")).unwrap(),
            data_bytes()
        );
    }

    #[test]
    fn a_server_that_never_answers_fails_after_six_attempts() {
        let (files, specs) = fake_model();
        let server = serve(
            files,
            Behavior {
                always_drop: true,
                ..Behavior::default()
            },
        );
        let dir = crate::audio::temp_dir("download-dead").join("m");
        let err = run(&dir, &specs, &server, &AtomicBool::new(false), &no_progress).unwrap_err();
        assert!(err.contains("encoder.int8.onnx.data"), "{err}");
        assert_eq!(server.log.lock().unwrap().len(), 6);
        assert!(!dir.join(crate::asr::model::READY_MARKER).exists());
    }

    #[test]
    fn cancel_keeps_the_part_and_a_second_run_resumes() {
        let (files, specs) = fake_model();
        // Only 30,000 of the 100,000 bytes are ever sent before the cancel, so the part is
        // partial however the client's reads happen to be sized (a fast machine can deliver the
        // whole file in one read, which made this test fail now and then).
        let server = serve(
            files,
            Behavior {
                stall_after: 30_000,
                ..Behavior::default()
            },
        );
        let dir = crate::audio::temp_dir("download-cancel").join("m");
        let cancel = AtomicBool::new(false);
        let began = std::time::Instant::now();
        let err = run(&dir, &specs, &server, &cancel, &|done, _| {
            if done > 20_000 {
                cancel.store(true, Ordering::SeqCst);
            }
        })
        .unwrap_err();
        assert_eq!(err, "cancelled");
        // The server holds the connection open, so a read that ignored the cancel would wait
        // for the 5 s stall limit of `tuning()` before anything noticed.
        assert!(
            began.elapsed() < Duration::from_secs(4),
            "cancel was not prompt"
        );
        let part = std::fs::metadata(dir.join("encoder.int8.onnx.data.part"))
            .unwrap()
            .len();
        assert!(part > 0 && part <= 30_000, "part is {part} bytes");
        assert!(!dir.join(crate::asr::model::READY_MARKER).exists());

        cancel.store(false, Ordering::SeqCst);
        run(&dir, &specs, &server, &cancel, &no_progress).unwrap();
        assert_eq!(
            std::fs::read(dir.join("encoder.int8.onnx.data")).unwrap(),
            data_bytes()
        );
        let log = server.log.lock().unwrap();
        assert!(
            log.iter().any(|(_, r)| r.is_some()),
            "second run did not resume: {log:?}"
        );
    }

    #[test]
    fn an_installed_model_is_not_downloaded_again() {
        let (files, specs) = fake_model();
        let server = serve(files, Behavior::default());
        let dir = crate::audio::temp_dir("download-again").join("m");
        run(&dir, &specs, &server, &AtomicBool::new(false), &no_progress).unwrap();
        let before = server.log.lock().unwrap().len();
        let last = Mutex::new((0, 0));
        run(&dir, &specs, &server, &AtomicBool::new(false), &|d, t| {
            *last.lock().unwrap() = (d, t);
        })
        .unwrap();
        assert_eq!(server.log.lock().unwrap().len(), before, "no new requests");
        let (d, t) = *last.lock().unwrap();
        assert!(t > 0 && d == t);
    }

    #[test]
    fn a_missing_file_on_the_server_is_an_error() {
        let (mut files, specs) = fake_model();
        files.remove("LICENSE");
        let server = serve(files, Behavior::default());
        let dir = crate::audio::temp_dir("download-404").join("m");
        let err = run(&dir, &specs, &server, &AtomicBool::new(false), &no_progress).unwrap_err();
        assert!(err.contains("LICENSE"), "{err}");
    }
}
