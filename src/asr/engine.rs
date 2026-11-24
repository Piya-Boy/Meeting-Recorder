//! Speech to text: the OpenThai 2.0 ASR Streaming model run with ONNX Runtime.
//!
//! A FastConformer encoder reads a whole segment at once, then a greedy RNN-T decoder emits
//! the pieces. Ported from PoodPim `src/asr.rs` (commit 15e07cc, MIT, Copyright (c) 2026
//! Thirawat Sutalungka, https://github.com/thirawat/PoodPim; see THIRD_PARTY_NOTICES.md):
//! tensor names, the language prompts, the blank bias and the pause-to-space rule. Beam
//! search, hotwords and the int8 decoder are not ported (a Python trial of beam search and
//! hotwords on a real recording did not pay off: `DECISIONS.md`).

use std::path::Path;
use std::sync::{Arc, Mutex};

use ort::session::builder::{AutoDevicePolicy, GraphOptimizationLevel};
use ort::session::Session;
use ort::value::{Tensor, TensorRef};

use crate::asr::features::{log_mel, N_MELS, SAMPLE_RATE};
use crate::asr::segment::{quiet_frames, QUIET_FRAME_MS};
use crate::asr::thai;

/// How the model writes: Thai letters only, or English words in Latin letters.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Language {
    #[default]
    Thai,
    ThaiEnglish,
}

impl Language {
    /// In the order of the Settings select.
    pub const ALL: [Language; 2] = [Language::Thai, Language::ThaiEnglish];

    /// The model's `prompt_index` input (from the model card: 32 Thai, 63 Thai mixed with English).
    pub fn prompt_index(self) -> i64 {
        match self {
            Language::Thai => 32,
            Language::ThaiEnglish => 63,
        }
    }

    pub fn index(self) -> usize {
        Language::ALL.iter().position(|l| *l == self).unwrap_or(0)
    }

    /// The language at `index` in the select; an unknown index is Thai.
    pub fn from_index(index: usize) -> Language {
        Language::ALL.get(index).copied().unwrap_or_default()
    }

    /// The name stored in the settings file.
    pub fn code(self) -> &'static str {
        match self {
            Language::Thai => "th",
            Language::ThaiEnglish => "th-en",
        }
    }

    /// An unknown name is Thai.
    pub fn from_code(code: &str) -> Language {
        Language::ALL
            .into_iter()
            .find(|l| l.code() == code)
            .unwrap_or_default()
    }
}

/// Turns a segment of 16 kHz mono audio into text.
pub trait Recognizer: Send {
    /// The text said in `samples_16k`, normalized; empty when nothing was said.
    fn recognize(&mut self, samples_16k: &[f32]) -> Result<String, String>;

    /// The language the next segments are written in (Thai until set).
    fn set_language(&mut self, _language: Language) {}
}

const ENC_DIM: usize = 1024;
const STATE_LEN: usize = 2 * 640; // [2, 1, 640]
const MAX_SYMBOLS_PER_FRAME: usize = 10;
/// Frames (80 ms each) the model must wait between two pieces for a pause to count.
const PAUSE_FRAMES: u32 = 8;
/// The shortest silence (ms) between two words that becomes a space. 300 ms also spaced
/// thinking pauses inside phrases, 700 ms left almost none (PoodPim's tuning).
const PAUSE_MS: usize = 500;
/// Audio under this many samples (0.2 s) is not decoded.
const MIN_SAMPLES: usize = SAMPLE_RATE as usize / 5;
/// Marks a pause in the text while it is put together.
const PAUSE: char = '\u{1F}';

/// Ending particles: a pause before them is the speaker trailing off, not a new clause.
const PARTICLES: &[&str] = &[
    "ครับ",
    "ค่ะ",
    "คะ",
    "ค่า",
    "นะ",
    "น่ะ",
    "ฮะ",
    "ฮับ",
    "เนี่ย",
    "อะ",
    "อ่ะ",
    "จ้ะ",
    "จ้า",
    "แหละ",
    "ล่ะ",
    "มั้ย",
    "ไหม",
    "เหรอ",
    "หรอ",
];

/// Whether `text` starts with an ending particle as a word of its own, not the start of a
/// longer word: "อะไร", "คะแนน", "ค่าใช้จ่าย", "ไหม้" don't count.
fn starts_with_particle(text: &str) -> bool {
    PARTICLES.iter().any(|p| {
        let Some(after) = text.strip_prefix(p) else {
            return false;
        };
        let next = after.chars().next();
        // A vowel sign or tone mark next belongs to the same syllable ("ไหม้").
        let same_syllable = next.is_some_and(
            |c| matches!(c, '\u{0E31}' | '\u{0E33}'..='\u{0E3A}' | '\u{0E47}'..='\u{0E4E}'),
        );
        let longer_word = match *p {
            "อะ" => after.starts_with("ไร"),
            "คะ" => after.starts_with("แนน"),
            // ค่า at the start of a clause is nearly always the noun (ค่าไฟ, ค่าเฉลี่ย).
            "ค่า" => next.is_some_and(starts_thai_word),
            _ => false,
        };
        !same_syllable && !longer_word
    })
}

/// A Thai character a word can start with: not a vowel sign or tone mark that belongs to the
/// letter before it, and not ๆ.
fn starts_thai_word(c: char) -> bool {
    ('\u{0E01}'..='\u{0E30}').contains(&c) && c != '\u{0E30}'
        || ('\u{0E40}'..='\u{0E44}').contains(&c)
}

/// Turns pause marks into spaces, except before an ending particle.
fn pause_spaces(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(i) = rest.find(PAUSE) {
        out.push_str(&rest[..i]);
        rest = &rest[i + PAUSE.len_utf8()..];
        if !starts_with_particle(rest) {
            out.push(' ');
        }
    }
    out.push_str(rest);
    out
}

/// Whether the audio between encoder frames `a` and `b` (80 ms each) holds a silence of
/// `PAUSE_MS` or more. The model emits a piece a little after it is heard, so the window
/// starts 160 ms before `a`. `quiet`: silent 20 ms frames.
fn silence_between(quiet: &[bool], a: u32, b: u32) -> bool {
    let q = QUIET_FRAME_MS;
    let from = ((a as usize * 80).saturating_sub(160) / q).min(quiet.len());
    let to = ((b as usize * 80) / q).min(quiet.len());
    let need = PAUSE_MS.div_ceil(q);
    let mut run = 0;
    for &s in quiet.get(from..to).unwrap_or(&[]) {
        run = if s { run + 1 } else { 0 };
        if run >= need {
            return true;
        }
    }
    false
}

/// Pieces to text. `ids` index `tokens` (an id past the table is the blank and writes
/// nothing), `at[k]` is the encoder frame piece `k` was emitted at. A wait of `PAUSE_FRAMES`
/// or more over real silence between two Thai pieces becomes a space (not before an ending
/// particle). The result is normalized.
fn decode_tokens(ids: &[usize], at: &[u32], quiet: &[bool], tokens: &[String]) -> String {
    let mut text = String::new();
    for (k, piece) in ids.iter().filter_map(|&i| tokens.get(i)).enumerate() {
        if k > 0
            && at.get(k).zip(at.get(k - 1)).is_some_and(|(now, before)| {
                now.saturating_sub(*before) >= PAUSE_FRAMES && silence_between(quiet, *before, *now)
            })
        {
            let next = piece.chars().next();
            let prev = text.chars().next_back();
            if next.is_some_and(starts_thai_word)
                && prev.is_some_and(|c| ('\u{0E01}'..='\u{0E5B}').contains(&c))
            {
                text.push(PAUSE);
            }
        }
        text.push_str(piece);
    }
    let text = pause_spaces(&text).replace('\u{2581}', " ");
    thai::normalize(&text)
}

fn too_short(samples: usize) -> bool {
    samples < MIN_SAMPLES
}

/// Score added to the blank piece: short clips drop words when the model is too eager to
/// emit nothing, so it is held back harder under 2 s.
fn blank_bias(samples: usize) -> f32 {
    if samples < 2 * SAMPLE_RATE as usize {
        -2.5
    } else {
        -1.0
    }
}

/// ONNX Runtime allows one environment per process, so every load shares this one.
fn environment() -> Result<Arc<ort::environment::Environment>, ort::Error> {
    static ENV: Mutex<Option<Arc<ort::environment::Environment>>> = Mutex::new(None);
    let mut env = ENV.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(e) = env.as_ref() {
        return Ok(e.clone());
    }
    let e = Arc::new(ort::init().with_name("meeting-recorder").build()?);
    *env = Some(e.clone());
    Ok(e)
}

/// The model on ONNX Runtime (CPU).
pub struct OrtRecognizer {
    _env: Arc<ort::environment::Environment>,
    encoder: Session,
    decoder: Session,
    tokens: Vec<String>,
    blank: usize,
    prompt: i64,
}

impl OrtRecognizer {
    /// Loads the model in `dir` (see `asr::model`): the int8 encoder if it is there, else the
    /// fp32 one. `threads` is the ONNX Runtime thread count for the encoder; the decoder uses
    /// at most 4 (more did not help in the 4/8-thread check on this project's hardware).
    pub fn load(dir: &Path, threads: usize) -> Result<OrtRecognizer, String> {
        let encoder_file = ["encoder.int8.onnx", "encoder.onnx"]
            .iter()
            .map(|f| dir.join(f))
            .find(|p| p.is_file() && p.with_extension("onnx.data").is_file())
            .ok_or_else(|| format!("no encoder model in {}", dir.display()))?;
        let tokens: Vec<String> = std::fs::read_to_string(crate::asr::model::tokens_path(dir))
            .map_err(|e| format!("cannot read tokens.txt: {e}"))?
            .lines()
            .map(str::to_owned)
            .collect();
        let blank = tokens.len();
        let threads = threads.max(1);
        let env = environment().map_err(|e| e.to_string())?;
        let build = |path: &Path, n: usize| -> Result<Session, String> {
            // One step at a time: the builder's error type is large to carry through closures.
            let b = Session::builder(&env).map_err(|e| e.to_string())?;
            let b = b
                .with_auto_device(AutoDevicePolicy::PreferCPU)
                .map_err(|e| e.to_string())?;
            let b = b
                .with_optimization_level(GraphOptimizationLevel::All)
                .map_err(|e| e.to_string())?;
            let mut b = b.with_intra_threads(n).map_err(|e| e.to_string())?;
            b.commit_from_file(path)
                .map_err(|e| format!("cannot load {}: {e}", path.display()))
        };
        let encoder = build(&encoder_file, threads)?;
        let decoder = build(&crate::asr::model::decoder_path(dir), threads.min(4))?;
        Ok(OrtRecognizer {
            _env: env,
            encoder,
            decoder,
            tokens,
            blank,
            prompt: Language::Thai.prompt_index(),
        })
    }

    fn run(&mut self, audio: &[f32]) -> Result<String, ort::Error> {
        let (feats, t) = log_mel(audio);
        let enc_out = self.encoder.run(ort::inputs![
            "audio_signal" => Tensor::from_array(([1usize, N_MELS, t], feats))?,
            "length" => Tensor::from_array(([1usize], vec![t as i64]))?,
            "prompt_index" => Tensor::from_array(([1usize], vec![self.prompt]))?,
        ])?;
        let (shape, data) = enc_out["encoded"].try_extract_tensor::<f32>()?;
        let (_, enc_len) = enc_out["encoded_length"].try_extract_tensor::<i64>()?;
        let frames = (shape[1] as usize).min(enc_len[0] as usize);
        let encoded: Vec<f32> = data[..frames * ENC_DIM].to_vec();
        drop(enc_out);
        let (ids, at) = self.greedy(&encoded, frames, blank_bias(audio.len()))?;
        let quiet = quiet_frames(audio, SAMPLE_RATE as usize);
        Ok(decode_tokens(&ids, &at, &quiet, &self.tokens))
    }

    /// Greedy RNN-T decoding: the piece ids and the encoder frame each was emitted at.
    fn greedy(
        &mut self,
        enc: &[f32],
        frames: usize,
        blank_bias: f32,
    ) -> Result<(Vec<usize>, Vec<u32>), ort::Error> {
        let mut s1 = vec![0.0f32; STATE_LEN];
        let mut s2 = vec![0.0f32; STATE_LEN];
        let mut last = [self.blank as i32];
        let one = [1i32];
        let mut out = Vec::new();
        let mut at = Vec::new();
        for t in 0..frames {
            let frame = &enc[t * ENC_DIM..(t + 1) * ENC_DIM]; // [1, 1024, 1]
            for _ in 0..MAX_SYMBOLS_PER_FRAME {
                let res = self.decoder.run(ort::inputs![
                    "encoder_outputs" => TensorRef::from_array_view(([1usize, ENC_DIM, 1], frame))?,
                    "targets" => TensorRef::from_array_view(([1usize, 1], &last[..]))?,
                    "target_length" => TensorRef::from_array_view(([1usize], &one[..]))?,
                    "input_states_1" => TensorRef::from_array_view(([2usize, 1, 640], &s1[..]))?,
                    "input_states_2" => TensorRef::from_array_view(([2usize, 1, 640], &s2[..]))?,
                ])?;
                let (_, scores) = res["outputs"].try_extract_tensor::<f32>()?;
                let mut best = 0usize;
                let mut best_v = f32::NEG_INFINITY;
                for (i, &v) in scores.iter().enumerate() {
                    let v = if i == self.blank { v + blank_bias } else { v };
                    if v > best_v {
                        best_v = v;
                        best = i;
                    }
                }
                if best == self.blank {
                    break;
                }
                out.push(best);
                at.push(t as u32);
                last[0] = best as i32;
                s1.copy_from_slice(res["output_states_1"].try_extract_tensor::<f32>()?.1);
                s2.copy_from_slice(res["output_states_2"].try_extract_tensor::<f32>()?.1);
            }
        }
        Ok((out, at))
    }
}

impl Recognizer for OrtRecognizer {
    fn recognize(&mut self, samples_16k: &[f32]) -> Result<String, String> {
        if too_short(samples_16k.len()) {
            return Ok(String::new());
        }
        self.run(samples_16k).map_err(|e| e.to_string())
    }

    fn set_language(&mut self, language: Language) {
        self.prompt = language.prompt_index();
    }
}

/// A recognizer for tests: canned answers in order, then empty text; records the length of
/// every input.
#[cfg(test)]
pub(crate) struct FakeRecognizer {
    answers: std::collections::VecDeque<String>,
    error: Option<String>,
    pub lengths: Arc<Mutex<Vec<usize>>>,
}

#[cfg(test)]
impl FakeRecognizer {
    pub fn new(answers: &[&str]) -> FakeRecognizer {
        FakeRecognizer {
            answers: answers.iter().map(|s| (*s).to_owned()).collect(),
            error: None,
            lengths: Arc::default(),
        }
    }

    pub fn failing(message: &str) -> FakeRecognizer {
        FakeRecognizer {
            error: Some(message.to_owned()),
            ..FakeRecognizer::new(&[])
        }
    }
}

#[cfg(test)]
impl Recognizer for FakeRecognizer {
    fn recognize(&mut self, samples_16k: &[f32]) -> Result<String, String> {
        self.lengths.lock().unwrap().push(samples_16k.len());
        if let Some(e) = &self.error {
            return Err(e.clone());
        }
        Ok(self.answers.pop_front().unwrap_or_default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Token table of 6 pieces; the blank would be id 6.
    fn table() -> Vec<String> {
        ["\u{2581}", "ก", "า", "ร", "ครับ", "ไป"]
            .iter()
            .map(|s| (*s).to_owned())
            .collect()
    }

    fn loud(frames: usize) -> Vec<bool> {
        vec![false; frames]
    }

    fn quiet(frames: usize) -> Vec<bool> {
        vec![true; frames]
    }

    #[test]
    fn fake_recognizer_answers_in_order_and_records_lengths() {
        let mut f = FakeRecognizer::new(&["a", "b"]);
        assert_eq!(f.recognize(&[0.0; 10]).unwrap(), "a");
        assert_eq!(f.recognize(&[0.0; 20]).unwrap(), "b");
        assert_eq!(f.recognize(&[0.0; 30]).unwrap(), "");
        assert_eq!(*f.lengths.lock().unwrap(), vec![10, 20, 30]);
    }

    #[test]
    fn fake_recognizer_can_fail() {
        let mut f = FakeRecognizer::failing("boom");
        assert_eq!(f.recognize(&[0.0; 10]).unwrap_err(), "boom");
    }

    #[test]
    fn pieces_join_into_text() {
        assert_eq!(decode_tokens(&[1, 2, 3], &[0, 1, 2], &[], &table()), "การ");
    }

    #[test]
    fn word_start_mark_becomes_a_space_and_the_blank_is_skipped() {
        // id 6 is the blank (past the table): no text
        assert_eq!(
            decode_tokens(&[1, 0, 2, 6], &[0, 1, 2, 3], &[], &table()),
            "ก า"
        );
    }

    #[test]
    fn split_sara_am_is_combined_in_the_result() {
        let tokens: Vec<String> = ["ท", "\u{0E4D}", "\u{0E32}"]
            .iter()
            .map(|s| (*s).to_owned())
            .collect();
        assert_eq!(decode_tokens(&[0, 1, 2], &[0, 1, 2], &[], &tokens), "ทำ");
    }

    #[test]
    fn a_long_wait_over_real_silence_becomes_a_space() {
        // ก at frame 0, ร at frame 20 (80 ms frames); 20 ms quiet frames all quiet
        assert_eq!(
            decode_tokens(&[1, 3], &[0, 20], &quiet(500), &table()),
            "ก ร"
        );
    }

    #[test]
    fn a_short_wait_gives_no_space() {
        assert_eq!(decode_tokens(&[1, 3], &[0, 5], &quiet(500), &table()), "กร");
    }

    #[test]
    fn a_long_wait_over_sound_gives_no_space() {
        assert_eq!(decode_tokens(&[1, 3], &[0, 20], &loud(500), &table()), "กร");
    }

    #[test]
    fn a_pause_before_an_ending_particle_gives_no_space() {
        assert_eq!(
            decode_tokens(&[1, 4], &[0, 20], &quiet(500), &table()),
            "กครับ"
        );
    }

    #[test]
    fn particles_are_told_from_longer_words() {
        assert!(starts_with_particle("ครับ"));
        assert!(starts_with_particle("นะครับ"));
        assert!(!starts_with_particle("อะไร"));
        assert!(!starts_with_particle("คะแนน"));
        assert!(!starts_with_particle("ไหม้"));
        assert!(!starts_with_particle("ค่าไฟ"));
        assert!(!starts_with_particle("ก"));
    }

    #[test]
    fn short_audio_is_below_the_decoding_limit() {
        assert!(too_short(0));
        assert!(too_short(3_199));
        assert!(!too_short(3_200));
    }

    #[test]
    fn blank_is_penalised_harder_for_short_audio() {
        assert_eq!(blank_bias(30_400), -2.5); // 1.9 s
        assert_eq!(blank_bias(32_000), -1.0); // 2.0 s
        assert_eq!(blank_bias(160_000), -1.0);
    }

    #[test]
    fn missing_model_files_fail_loading_with_a_message() {
        let dir = crate::audio::temp_dir("engine-missing");
        let err = OrtRecognizer::load(&dir, 4).err().unwrap();
        assert!(err.contains("encoder"), "{err}");
    }

    // Model-dependent checks: run with
    //   ASR_MODEL_DIR=<folder with encoder.int8.onnx(+.data), decoder_joint.onnx, tokens.txt>
    //   ASR_TEST_CLIPS=<folder with <name>.wav (16 kHz mono) and <name>.txt references>
    //   cargo test asr::engine -- --ignored --nocapture

    fn env_dir(name: &str) -> std::path::PathBuf {
        std::path::PathBuf::from(
            std::env::var_os(name).unwrap_or_else(|| panic!("set {name} to run this test")),
        )
    }

    #[test]
    #[ignore = "needs the model (ASR_MODEL_DIR) and clips (ASR_TEST_CLIPS)"]
    fn loads_and_transcribes_a_clip() {
        let mut rec = OrtRecognizer::load(&env_dir("ASR_MODEL_DIR"), 4).unwrap();
        let clip = std::fs::read_dir(env_dir("ASR_TEST_CLIPS"))
            .unwrap()
            .filter_map(Result::ok)
            .map(|e| e.path())
            .find(|p| p.extension().is_some_and(|x| x == "wav"))
            .expect("a wav clip");
        let mut reader = crate::asr::wavread::WavReader::open(&clip).unwrap();
        let audio = reader.read_block(usize::MAX / 4).unwrap();
        assert_eq!(reader.rate(), 16_000);
        let text = rec.recognize(&audio).unwrap();
        println!("{text}");
        assert!(text.chars().any(|c| ('\u{0E01}'..='\u{0E5B}').contains(&c)));
    }

    #[test]
    fn each_language_has_the_model_prompt_and_a_settings_index() {
        assert_eq!(Language::Thai.prompt_index(), 32);
        assert_eq!(Language::ThaiEnglish.prompt_index(), 63);
        assert_eq!(Language::default(), Language::Thai);
        for (i, l) in Language::ALL.iter().enumerate() {
            assert_eq!(Language::from_index(i), *l);
            assert_eq!(l.index(), i);
            assert_eq!(Language::from_code(l.code()), *l);
        }
        assert_eq!(Language::from_index(9), Language::Thai);
        assert_eq!(Language::from_code("??"), Language::Thai);
    }
}
