//! The OpenThai ASR model files: what they are, where they live, whether they are installed.
//!
//! The model is not shipped with the app: it is downloaded on first use from a pinned revision
//! of the model repository, and every file is checked against the SHA-256 listed here. The
//! download itself is in `download` (next to this code in a later step). File names, sizes and
//! hashes were read from the Hugging Face tree API at the pinned revision; the layout follows
//! PoodPim's `src/model.rs` (commit 15e07cc, MIT, Copyright (c) 2026 Thirawat Sutalungka,
//! https://github.com/thirawat/PoodPim; see THIRD_PARTY_NOTICES.md).

use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

/// Revision of the model repository every file is taken from.
pub const REVISION: &str = "9f142a43e4d35a422c1c7dac40772ef707387ebc";
/// Where the files of `REVISION` are downloaded from (append `/<file name>`).
pub const BASE_URL: &str = "https://huggingface.co/iapp/OpenThai2.0-ASR-Streaming-ONNX/resolve/9f142a43e4d35a422c1c7dac40772ef707387ebc";
/// Written after a download was verified and unpacked; `status` trusts it instead of hashing
/// a 1 GB file on every start.
pub const READY_MARKER: &str = "ready.ok";

/// Weights of the model. int8 is the default: on this project's 10-clip check it was as
/// accurate as fp32 within half a point, at half the memory and about 1.4 times the speed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Precision {
    Int8,
    Fp32,
}

impl Precision {
    pub fn as_str(self) -> &'static str {
        match self {
            Precision::Int8 => "int8",
            Precision::Fp32 => "fp32",
        }
    }

    /// `"fp32"` is fp32; anything else (including an empty or unknown setting) is int8.
    pub fn from_str_lossy(s: &str) -> Precision {
        if s == "fp32" {
            Precision::Fp32
        } else {
            Precision::Int8
        }
    }
}

/// A file to download.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ManifestFile {
    pub name: &'static str,
    pub size: u64,
    pub sha256: &'static str,
}

const LICENSE_FILE: ManifestFile = ManifestFile {
    name: "LICENSE",
    size: 2_619,
    sha256: "116493ed1369502b5e29cc559a8fa009a371bae926e769038ebb09885b091221",
};

/// The files of a precision: the encoder weights, the archive (encoder graph, decoder,
/// tokens) and the model's licence.
pub fn manifest(p: Precision) -> [ManifestFile; 3] {
    match p {
        Precision::Int8 => [
            ManifestFile {
                name: "encoder.int8.onnx.data",
                size: 883_175_424,
                sha256: "39176685a17087bc45a2a002cdc760951ccc4781693780a59db2b2a05ae964a5",
            },
            ManifestFile {
                name: "openthai2.0-asr-streaming-int8.asr.zip",
                size: 109_014_005,
                sha256: "6714a3333dbbef388aab0a0623a73da19953d9473dc59b98a77dda1a48911553",
            },
            LICENSE_FILE,
        ],
        Precision::Fp32 => [
            ManifestFile {
                name: "encoder.onnx.data",
                size: 2_454_405_120,
                sha256: "a139201333ab89d5e965bc12abb1c534aa285d82e40051c2dda605365c74f18b",
            },
            ManifestFile {
                name: "openthai2.0-asr-streaming-fp32.asr.zip",
                size: 149_365_631,
                sha256: "d4161d14e1c61c779ae01cf02356a99d2752f7931ce2d04be8098c57270d463f",
            },
            LICENSE_FILE,
        ],
    }
}

const DECODER: &str = "decoder_joint.onnx";
const TOKENS: &str = "tokens.txt";
/// Files taken out of the archive; anything else in it is ignored.
const ARCHIVE_FILES: [&str; 4] = ["encoder.int8.onnx", "encoder.onnx", DECODER, TOKENS];

/// The encoder graph of a precision (its weights file is `<name>.data` beside it).
pub fn encoder_path(dir: &Path, p: Precision) -> PathBuf {
    dir.join(match p {
        Precision::Int8 => "encoder.int8.onnx",
        Precision::Fp32 => "encoder.onnx",
    })
}

pub fn decoder_path(dir: &Path) -> PathBuf {
    dir.join(DECODER)
}

pub fn tokens_path(dir: &Path) -> PathBuf {
    dir.join(TOKENS)
}

/// `<base>\<revision, 7 characters>\<precision>`.
pub fn model_dir_in(base: &Path, p: Precision) -> PathBuf {
    base.join(&REVISION[..7]).join(p.as_str())
}

/// `%LOCALAPPDATA%\MeetingRecorder\models\<revision>\<precision>`; the temp folder when
/// `LOCALAPPDATA` is not set.
pub fn model_dir(p: Precision) -> PathBuf {
    let root = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    model_dir_in(&root.join("MeetingRecorder").join("models"), p)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ModelStatus {
    Missing,
    Downloading { done: u64, total: u64 },
    Verifying,
    Ready,
    Failed(String),
}

/// Whether the model of `p` is installed in `dir`: the marker of a verified install is there,
/// the weights file has its full size and the other files exist. Never reads the weights.
pub fn status(dir: &Path, p: Precision) -> ModelStatus {
    let data = manifest(p)[0];
    let data_ok = fs::metadata(dir.join(data.name)).is_ok_and(|m| m.len() == data.size);
    let ready = dir.join(READY_MARKER).is_file()
        && data_ok
        && encoder_path(dir, p).is_file()
        && decoder_path(dir).is_file()
        && tokens_path(dir).is_file();
    if ready {
        ModelStatus::Ready
    } else {
        ModelStatus::Missing
    }
}

/// Records that the files in `dir` were verified.
pub fn mark_ready(dir: &Path) -> io::Result<()> {
    fs::write(dir.join(READY_MARKER), b"ok")
}

/// SHA-256 of a file as lowercase hex; `None` when `cancel` was set while reading.
pub fn sha256_file(path: &Path, cancel: &AtomicBool) -> io::Result<Option<String>> {
    use sha2::{Digest, Sha256};
    let mut file = File::open(path)?;
    let mut hash = Sha256::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Ok(None);
        }
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hash.update(&buf[..n]);
    }
    Ok(Some(
        hash.finalize().iter().map(|b| format!("{b:02x}")).collect(),
    ))
}

/// Unpacks the model files of a downloaded archive into `dir`. Only the file name of each
/// entry is used, entries that would leave `dir` are skipped and only known model files are
/// written, each through `<name>.part` so a file cut short is never taken for a finished one.
pub fn extract_zip(zip_path: &Path, dir: &Path) -> Result<(), String> {
    let file = File::open(zip_path).map_err(|e| format!("cannot open the model archive: {e}"))?;
    let mut archive =
        zip::ZipArchive::new(file).map_err(|e| format!("not a valid model archive: {e}"))?;
    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| format!("cannot read the model archive: {e}"))?;
        if entry.is_dir() {
            continue;
        }
        let Some(name) = entry
            .enclosed_name()
            .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
        else {
            continue;
        };
        if !ARCHIVE_FILES.contains(&name.as_str()) {
            continue;
        }
        let dest = dir.join(&name);
        let part = dir.join(format!("{name}.part"));
        let result = (|| {
            let mut out = File::create(&part)?;
            io::copy(&mut entry, &mut out)?;
            out.sync_all()?;
            fs::rename(&part, &dest)
        })();
        if let Err(e) = result {
            let _ = fs::remove_file(&part);
            return Err(format!("cannot write {name}: {e}"));
        }
    }
    let has_encoder = dir.join("encoder.int8.onnx").is_file() || dir.join("encoder.onnx").is_file();
    if !has_encoder {
        return Err("the encoder graph is missing from the model archive".to_owned());
    }
    for name in [DECODER, TOKENS] {
        if !dir.join(name).is_file() {
            return Err(format!("{name} is missing from the model archive"));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write as _;
    use std::sync::atomic::AtomicBool;

    #[test]
    fn manifest_has_the_pinned_sizes_and_hashes() {
        let int8 = manifest(Precision::Int8);
        assert_eq!(int8[0].name, "encoder.int8.onnx.data");
        assert_eq!(int8[0].size, 883_175_424);
        assert_eq!(
            int8[0].sha256,
            "39176685a17087bc45a2a002cdc760951ccc4781693780a59db2b2a05ae964a5"
        );
        assert_eq!(int8[1].name, "openthai2.0-asr-streaming-int8.asr.zip");
        assert_eq!(int8[1].size, 109_014_005);
        assert_eq!(
            int8[1].sha256,
            "6714a3333dbbef388aab0a0623a73da19953d9473dc59b98a77dda1a48911553"
        );
        assert_eq!(int8[2].name, "LICENSE");
        assert_eq!(int8[2].size, 2_619);
        assert_eq!(
            int8[2].sha256,
            "116493ed1369502b5e29cc559a8fa009a371bae926e769038ebb09885b091221"
        );

        let fp32 = manifest(Precision::Fp32);
        assert_eq!(fp32[0].name, "encoder.onnx.data");
        assert_eq!(fp32[0].size, 2_454_405_120);
        assert_eq!(
            fp32[0].sha256,
            "a139201333ab89d5e965bc12abb1c534aa285d82e40051c2dda605365c74f18b"
        );
        assert_eq!(fp32[1].name, "openthai2.0-asr-streaming-fp32.asr.zip");
        assert_eq!(fp32[1].size, 149_365_631);
        assert_eq!(
            fp32[1].sha256,
            "d4161d14e1c61c779ae01cf02356a99d2752f7931ce2d04be8098c57270d463f"
        );
        assert_eq!(fp32[2].name, "LICENSE");
    }

    #[test]
    fn base_url_is_the_pinned_revision() {
        assert_eq!(
            BASE_URL,
            "https://huggingface.co/iapp/OpenThai2.0-ASR-Streaming-ONNX/resolve/9f142a43e4d35a422c1c7dac40772ef707387ebc"
        );
        assert!(BASE_URL.ends_with(REVISION));
    }

    #[test]
    fn precision_round_trips_and_falls_back_to_int8() {
        assert_eq!(Precision::Int8.as_str(), "int8");
        assert_eq!(Precision::Fp32.as_str(), "fp32");
        assert_eq!(Precision::from_str_lossy("fp32"), Precision::Fp32);
        assert_eq!(Precision::from_str_lossy("int8"), Precision::Int8);
        assert_eq!(Precision::from_str_lossy(""), Precision::Int8);
        assert_eq!(Precision::from_str_lossy("x"), Precision::Int8);
    }

    #[test]
    fn model_dir_names_revision_and_precision() {
        let base = std::path::Path::new("base");
        let dir = model_dir_in(base, Precision::Int8);
        assert!(dir.starts_with(base));
        assert!(dir.ends_with("9f142a4/int8"), "{dir:?}");
        assert!(model_dir_in(base, Precision::Fp32).ends_with("9f142a4/fp32"));
        assert!(model_dir(Precision::Int8).ends_with("9f142a4/int8"));
    }

    #[test]
    fn sha256_of_abc() {
        let dir = crate::audio::temp_dir("model-sha");
        let path = dir.join("f");
        std::fs::write(&path, b"abc").unwrap();
        let cancel = AtomicBool::new(false);
        assert_eq!(
            sha256_file(&path, &cancel).unwrap().as_deref(),
            Some("ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad")
        );
    }

    #[test]
    fn sha256_stops_when_cancelled() {
        let dir = crate::audio::temp_dir("model-sha-cancel");
        let path = dir.join("f");
        std::fs::write(&path, b"abc").unwrap();
        let cancel = AtomicBool::new(true);
        assert_eq!(sha256_file(&path, &cancel).unwrap(), None);
    }

    #[test]
    fn sha256_of_a_missing_file_is_an_error() {
        let cancel = AtomicBool::new(false);
        assert!(sha256_file(std::path::Path::new("no/such/file"), &cancel).is_err());
    }

    fn make_zip(path: &std::path::Path, entries: &[(&str, &[u8])]) {
        let file = std::fs::File::create(path).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        let opts = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        for (name, body) in entries {
            zip.start_file(*name, opts).unwrap();
            zip.write_all(body).unwrap();
        }
        zip.finish().unwrap();
    }

    #[test]
    fn extract_writes_the_files_and_skips_hostile_names() {
        let base = crate::audio::temp_dir("model-zip");
        let dir = base.join("out");
        std::fs::create_dir_all(&dir).unwrap();
        let zip = base.join("m.zip");
        make_zip(
            &zip,
            &[
                ("encoder.int8.onnx", b"enc"),
                ("decoder_joint.onnx", b"dec"),
                ("tokens.txt", b"tok"),
                ("../evil.txt", b"bad"),
            ],
        );
        extract_zip(&zip, &dir).unwrap();
        assert_eq!(
            std::fs::read(dir.join("encoder.int8.onnx")).unwrap(),
            b"enc"
        );
        assert_eq!(
            std::fs::read(dir.join("decoder_joint.onnx")).unwrap(),
            b"dec"
        );
        assert_eq!(std::fs::read(dir.join("tokens.txt")).unwrap(), b"tok");
        assert!(!base.join("evil.txt").exists());
        assert!(!dir.join("evil.txt").exists());
        let parts: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|e| e.path().extension().is_some_and(|x| x == "part"))
            .collect();
        assert!(parts.is_empty(), "{parts:?}");
    }

    #[test]
    fn extract_flattens_nested_names() {
        let base = crate::audio::temp_dir("model-zip-nested");
        let dir = base.join("out");
        std::fs::create_dir_all(&dir).unwrap();
        let zip = base.join("m.zip");
        make_zip(
            &zip,
            &[
                ("pack/encoder.onnx", b"enc"),
                ("pack/decoder_joint.onnx", b"dec"),
                ("pack/tokens.txt", b"tok"),
            ],
        );
        extract_zip(&zip, &dir).unwrap();
        assert!(dir.join("encoder.onnx").is_file());
        assert!(dir.join("tokens.txt").is_file());
    }

    #[test]
    fn extract_fails_naming_a_missing_file() {
        let base = crate::audio::temp_dir("model-zip-missing");
        let dir = base.join("out");
        std::fs::create_dir_all(&dir).unwrap();
        let zip = base.join("m.zip");
        make_zip(
            &zip,
            &[
                ("encoder.int8.onnx", b"enc"),
                ("decoder_joint.onnx", b"dec"),
            ],
        );
        let err = extract_zip(&zip, &dir).unwrap_err();
        assert!(err.contains("tokens.txt"), "{err}");
    }

    #[test]
    fn extract_fails_on_a_file_that_is_not_a_zip() {
        let base = crate::audio::temp_dir("model-zip-garbage");
        let zip = base.join("m.zip");
        std::fs::write(&zip, b"not a zip").unwrap();
        assert!(extract_zip(&zip, &base).is_err());
    }

    /// A model folder that looks installed, with a fake data file of the manifest's size
    /// replaced by `data_len` bytes.
    fn fake_install(dir: &std::path::Path, data_len: u64, marker: bool) {
        std::fs::create_dir_all(dir).unwrap();
        for f in ["encoder.int8.onnx", "decoder_joint.onnx", "tokens.txt"] {
            std::fs::write(dir.join(f), b"x").unwrap();
        }
        let data = std::fs::File::create(dir.join("encoder.int8.onnx.data")).unwrap();
        data.set_len(data_len).unwrap();
        if marker {
            mark_ready(dir).unwrap();
        }
    }

    #[test]
    fn status_of_an_empty_folder_is_missing() {
        let dir = crate::audio::temp_dir("model-status-empty");
        assert_eq!(status(&dir, Precision::Int8), ModelStatus::Missing);
        assert_eq!(
            status(&dir.join("nope"), Precision::Int8),
            ModelStatus::Missing
        );
    }

    #[test]
    fn status_is_ready_for_a_marked_complete_install() {
        let dir = crate::audio::temp_dir("model-status-ready");
        fake_install(&dir, 883_175_424, true);
        assert_eq!(status(&dir, Precision::Int8), ModelStatus::Ready);
    }

    #[test]
    fn status_needs_the_marker() {
        let dir = crate::audio::temp_dir("model-status-nomarker");
        fake_install(&dir, 883_175_424, false);
        assert_eq!(status(&dir, Precision::Int8), ModelStatus::Missing);
    }

    #[test]
    fn status_rejects_a_data_file_of_the_wrong_size() {
        let dir = crate::audio::temp_dir("model-status-size");
        fake_install(&dir, 1000, true);
        assert_eq!(status(&dir, Precision::Int8), ModelStatus::Missing);
    }

    #[test]
    fn status_rejects_a_missing_tokens_file() {
        let dir = crate::audio::temp_dir("model-status-tokens");
        fake_install(&dir, 883_175_424, true);
        std::fs::remove_file(dir.join("tokens.txt")).unwrap();
        assert_eq!(status(&dir, Precision::Int8), ModelStatus::Missing);
    }

    #[test]
    fn encoder_path_follows_precision() {
        let dir = std::path::Path::new("d");
        assert!(encoder_path(dir, Precision::Int8).ends_with("encoder.int8.onnx"));
        assert!(encoder_path(dir, Precision::Fp32).ends_with("encoder.onnx"));
    }
}
