//! UI language switching (English / Thai).
//!
//! English source strings are the keys: [`tr`] returns the Thai text when Thai is active and
//! the key has an entry, and otherwise returns the input unchanged. Strings that are not UI
//! copy (device names, file paths, mock transcript text) are therefore safe to pass through.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    En,
    Th,
}

static CURRENT: AtomicU8 = AtomicU8::new(0);

/// Sets the active UI language.
pub fn set_lang(lang: Lang) {
    CURRENT.store(lang as u8, Ordering::Relaxed);
}

/// Returns the active UI language.
pub fn lang() -> Lang {
    if CURRENT.load(Ordering::Relaxed) == Lang::Th as u8 {
        Lang::Th
    } else {
        Lang::En
    }
}

/// Maps the "Application language" dropdown index to a [`Lang`].
pub fn lang_from_index(idx: usize) -> Lang {
    if idx == 1 {
        Lang::Th
    } else {
        Lang::En
    }
}

/// Translates a UI string; unknown strings are returned as-is.
pub fn tr(s: &str) -> &str {
    if lang() == Lang::En {
        return s;
    }
    static MAP: OnceLock<HashMap<&'static str, &'static str>> = OnceLock::new();
    MAP.get_or_init(|| THAI.iter().copied().collect())
        .get(s)
        .copied()
        .unwrap_or(s)
}

/// Whether `s` has a Thai entry (for tests that must not touch the global language).
#[cfg(test)]
pub fn has_thai(s: &str) -> bool {
    THAI.iter().any(|(k, _)| *k == s)
}

/// Toast text for an action whose backend is not connected yet.
pub fn not_wired(what: &str) -> String {
    match lang() {
        Lang::En => format!("{what} is not wired up yet (UI preview)"),
        Lang::Th => format!("{} ยังไม่เชื่อมต่อ (ตัวอย่าง UI)", tr(what)),
    }
}

/// Toast text for an export format whose backend is not connected yet.
pub fn export_not_wired(fmt: &str) -> String {
    match lang() {
        Lang::En => format!("{fmt} export is not wired up yet (UI preview)"),
        Lang::Th => format!("การส่งออก {fmt} ยังไม่เชื่อมต่อ (ตัวอย่าง UI)"),
    }
}

/// English → Thai. Later duplicates of a key override earlier ones.
const THAI: &[(&str, &str)] = &[
    ("%", "%"),
    ("100% (Default)", "100% (ค่าเริ่มต้น)"),
    ("4 threads", "4 เธรด"),
    ("8 threads", "8 เธรด"),
    ("90%", "90%"),
    ("About", "เกี่ยวกับ"),
    ("About Meeting Recorder", "เกี่ยวกับ Meeting Recorder"),
    ("Accent color", "สีเน้น"),
    ("Actions", "การทำงาน"),
    ("Adjust UI and Thai transcript legibility scale.", "ปรับขนาดการแสดงผลของ UI และข้อความถอดเสียงภาษาไทย"),
    ("Advanced", "ขั้นสูง"),
    ("Advanced & About", "ขั้นสูงและเกี่ยวกับ"),
    ("All dates", "ทุกวันที่"),
    ("Amber", "เหลืองอำพัน"),
    ("Appearance", "รูปลักษณ์"),
    ("Appearance & Theme", "รูปลักษณ์และธีม"),
    ("Application language", "ภาษาของแอป"),
    ("Audio", "เสียง"),
    ("Audio Backend", "ระบบเสียง"),
    ("Audio (Windows WASAPI)", "เสียง (Windows WASAPI)"),
    ("Audio buffer interval before running incremental ASR decoding.", "ช่วงเวลาบัฟเฟอร์เสียงก่อนถอดเสียงแบบต่อเนื่อง"),
    ("Audio only", "เสียงอย่างเดียว"),
    ("Audio sources", "แหล่งเสียง"),
    ("Auto-scroll", "เลื่อนอัตโนมัติ"),
    ("Automatic saving", "บันทึกอัตโนมัติ"),
    ("Blue", "น้ำเงิน"),
    ("Browse", "เลือก"),
    ("Cancel", "ยกเลิก"),
    ("Channel configuration", "การตั้งค่าช่องสัญญาณ"),
    ("Clear cache", "ล้างแคช"),
    ("Click any paragraph below to edit text inline. Changes auto-save to SQLite.", "คลิกย่อหน้าใดก็ได้เพื่อแก้ไขข้อความ การเปลี่ยนแปลงจะบันทึกลง SQLite อัตโนมัติ"),
    ("Click any paragraph below to edit text inline. Changes auto-save to SQLite.", "คลิกย่อหน้าใดก็ได้เพื่อแก้ไขข้อความ การเปลี่ยนแปลงจะบันทึกลง SQLite อัตโนมัติ"),
    ("Close", "ปิด"),
    ("Collapse sidebar", "ย่อแถบด้านข้าง"),
    ("Configure Windows WASAPI loopback and microphone capture devices, format, and hardware tests.", "ตั้งค่าอุปกรณ์จับเสียง WASAPI loopback และไมโครโฟน รูปแบบ และการทดสอบฮาร์ดแวร์"),
    ("Configure local OpenThai ASR inference, ONNX Runtime threading, and live streaming behavior.", "ตั้งค่าการถอดเสียง OpenThai ASR ในเครื่อง เธรดของ ONNX Runtime และการสตรีมสด"),
    ("Configure startup behavior, system tray integration, and global shortcuts.", "ตั้งค่าการเริ่มต้นระบบ ถาดระบบ และคีย์ลัดส่วนกลาง"),
    ("Confirm before deleting recordings", "ยืนยันก่อนลบการบันทึก"),
    ("Connected", "เชื่อมต่อแล้ว"),
    ("Copy", "คัดลอก"),
    ("Copy all", "คัดลอกทั้งหมด"),
    ("Copy live transcript", "คัดลอกข้อความถอดเสียงสด"),
    ("Copy transcript", "คัดลอกข้อความถอดเสียง"),
    ("CPU thread configuration", "การตั้งค่าเธรด CPU"),
    ("Date", "วันที่"),
    ("Delete", "ลบ"),
    ("Delete recording", "ลบการบันทึก"),
    ("Delete recording?", "ลบการบันทึกนี้หรือไม่?"),
    ("Destination", "ปลายทาง"),
    ("Diagnostics export", "การส่งออกข้อมูลวินิจฉัย"),
    ("Diagnostics, log files, model cache, and runtime performance metrics.", "ข้อมูลวินิจฉัย ไฟล์บันทึก แคชโมเดล และตัวชี้วัดประสิทธิภาพ"),
    ("Disconnected", "ไม่ได้เชื่อมต่อ"),
    ("Display interim Thai text while speech is still being processed.", "แสดงข้อความไทยชั่วคราวขณะที่ยังประมวลผลเสียงอยู่"),
    ("Duration", "ความยาว"),
    ("Enable Microphone recording", "เปิดการบันทึกไมโครโฟน"),
    ("Enable System Audio recording", "เปิดการบันทึกเสียงระบบ"),
    ("Error", "ผิดพลาด"),
    ("Error · Device lost", "ผิดพลาด · อุปกรณ์หาย"),
    ("Execution backend (subject to model compatibility).", "ตัวประมวลผล (ขึ้นกับความเข้ากันได้ของโมเดล)"),
    ("Expand sidebar", "ขยายแถบด้านข้าง"),
    ("Export diagnostics", "ส่งออกข้อมูลวินิจฉัย"),
    ("Export JSON", "ส่งออก JSON"),
    ("Export SRT", "ส่งออก SRT"),
    ("Export TXT", "ส่งออก TXT"),
    ("File picker is not wired up yet (UI preview)", "ตัวเลือกไฟล์ยังไม่เชื่อมต่อ (ตัวอย่าง UI)"),
    ("Flush audio buffers and transcript segments to disk every 5 seconds.", "เขียนบัฟเฟอร์เสียงและข้อความถอดเสียงลงดิสก์ทุก 5 วินาที"),
    ("Folder picker is not wired up yet (UI preview)", "ตัวเลือกโฟลเดอร์ยังไม่เชื่อมต่อ (ตัวอย่าง UI)"),
    ("Font scaling", "ขนาดตัวอักษร"),
    ("Future feature", "ฟีเจอร์ในอนาคต"),
    ("General", "ทั่วไป"),
    ("General & Shortcuts", "ทั่วไปและคีย์ลัด"),
    ("Go to Recorder", "ไปที่หน้าบันทึก"),
    ("Green", "เขียว"),
    ("Hardware test is not wired up yet (UI preview)", "การทดสอบฮาร์ดแวร์ยังไม่เชื่อมต่อ (ตัวอย่าง UI)"),
    ("Help", "ช่วยเหลือ"),
    ("Idle", "ว่าง"),
    ("Interface display language (transcription language is set under Transcription).", "ภาษาที่ใช้แสดงผลของแอป (ภาษาถอดเสียงตั้งค่าในหน้าถอดเสียง)"),
    ("Intra-op worker threads allocated to ONNX inference.", "จำนวนเธรดภายใน operator สำหรับการประมวลผล ONNX"),
    ("Keep running in the Windows notification area when closing the main window.", "ให้แอปทำงานต่อในถาดระบบของ Windows เมื่อปิดหน้าต่างหลัก"),
    ("Keyboard shortcuts", "คีย์ลัด"),
    ("Keyboard shortcuts & Help", "คีย์ลัดและความช่วยเหลือ"),
    ("Language", "ภาษา"),
    ("Last 30 days", "30 วันล่าสุด"),
    ("Last 7 days", "7 วันล่าสุด"),
    ("Launch Meeting Recorder automatically when signing in to Windows 11.", "เปิด Meeting Recorder อัตโนมัติเมื่อเข้าสู่ระบบ Windows 11"),
    ("Light theme", "ธีมสว่าง"),
    ("Live transcript", "ถอดเสียงสด"),
    ("Local-first Thai speech recognition model.", "โมเดลรู้จำเสียงพูดภาษาไทยที่ทำงานในเครื่อง"),
    ("Log directory", "โฟลเดอร์บันทึก log"),
    ("Manage local folders for dual-track WAV recordings, transcripts, SQLite database, and temporary cache.", "จัดการโฟลเดอร์สำหรับไฟล์ WAV สองแทร็ก ข้อความถอดเสียง ฐานข้อมูล SQLite และแคชชั่วคราว"),
    ("Maximize", "ขยายเต็มจอ"),
    ("Microphone", "ไมโครโฟน"),
    ("Microphone input stream lost. Reconnect device and click Resume.", "สัญญาณไมโครโฟนขาดหาย เชื่อมต่ออุปกรณ์ใหม่แล้วกดเล่นต่อ"),
    ("Minimize", "ย่อหน้าต่าง"),
    ("Minimize to system tray", "ย่อลงถาดระบบ"),
    ("Model cache", "แคชโมเดล"),
    ("Model path", "ตำแหน่งโมเดล"),
    ("No recordings match your search.", "ไม่พบการบันทึกที่ตรงกับคำค้นหา"),
    ("No recordings yet", "ยังไม่มีการบันทึก"),
    ("No transcripts match your search.", "ไม่พบข้อความถอดเสียงที่ตรงกับคำค้นหา"),
    ("Open", "เปิด"),
    ("Open folder", "เปิดโฟลเดอร์"),
    ("Open transcript", "เปิดข้อความถอดเสียง"),
    ("Open-source licenses", "ไลเซนส์โอเพนซอร์ส"),
    ("Opening the log folder", "การเปิดโฟลเดอร์ log"),
    ("OpenThai ASR", "OpenThai ASR"),
    ("Optimize DB", "ปรับแต่ง DB"),
    ("Partial", "บางส่วน"),
    ("Partial transcription", "ถอดเสียงบางส่วน"),
    ("Pause", "พัก"),
    ("Pause / Resume recording", "หยุดชั่วคราว / เล่นต่อ"),
    ("Paused", "หยุดชั่วคราว"),
    ("Paused · 00:14:32", "หยุดชั่วคราว · 00:14:32"),
    ("Pending", "รอดำเนินการ"),
    ("Performance information", "ข้อมูลประสิทธิภาพ"),
    ("Preview", "ตัวอย่าง"),
    ("Preview empty state", "ดูสถานะว่างเปล่า"),
    ("Primary interactive highlight across buttons, toggles, and active navigation.", "สีเน้นหลักของปุ่ม สวิตช์ และเมนูที่เลือกอยู่"),
    ("Primary recognition language.", "ภาษาหลักที่ใช้รู้จำเสียง"),
    ("Processing", "ประมวลผล"),
    ("Processing · Finalizing", "กำลังประมวลผล · สรุปผล"),
    ("Record", "บันทึก"),
    ("Recorder", "บันทึกเสียง"),
    ("Recorder Idle", "ว่าง"),
    ("Recording", "กำลังบันทึก"),
    ("Recording · 00:14:32", "กำลังบันทึก · 00:14:32"),
    ("Recording directory", "โฟลเดอร์เก็บการบันทึก"),
    ("Recording file name", "ชื่อไฟล์บันทึก"),
    ("Recording name", "ชื่อการบันทึก"),
    ("Recordings", "การบันทึก"),
    ("Restore", "คืนขนาด"),
    ("Runtime", "รันไทม์"),
    ("Sample rate", "อัตราการสุ่มตัวอย่าง"),
    ("Search recordings by name...", "ค้นหาการบันทึกตามชื่อ..."),
    ("Search transcript...", "ค้นหาข้อความ..."),
    ("Search transcripts or Thai text (SQLite FTS)...", "ค้นหาข้อความถอดเสียงหรือคำไทย (SQLite FTS)..."),
    ("Select Dark, Light, or Windows 11 System theme, accent color, and UI font scaling.", "เลือกธีมมืด สว่าง หรือตามระบบ Windows 11 สีเน้น และขนาดตัวอักษร"),
    ("SESSION STATUS", "สถานะเซสชัน"),
    ("Separate audio tracks preserve System Audio and Microphone independently.", "แยกแทร็กเสียงเพื่อเก็บเสียงระบบและไมโครโฟนเป็นอิสระต่อกัน"),
    ("Separate tracks (2-ch)", "แยกแทร็ก (2 ช่อง)"),
    ("Settings", "ตั้งค่า"),
    ("Show a confirmation dialog before permanently removing audio files.", "แสดงหน้าต่างยืนยันก่อนลบไฟล์เสียงถาวร"),
    ("Show recordings list", "แสดงรายการบันทึก"),
    ("Size", "ขนาด"),
    ("Sort: Date (Newest)", "เรียง: วันที่ (ใหม่สุด)"),
    ("Sort: Date (Oldest)", "เรียง: วันที่ (เก่าสุด)"),
    ("Sort: Duration", "เรียง: ความยาว"),
    ("Sort: File size", "เรียง: ขนาดไฟล์"),
    ("Speaker diarization", "แยกผู้พูด"),
    ("Start / Stop recording", "เริ่ม / หยุดบันทึก"),
    ("Start a new recording session to capture System Audio and\nMicrophone tracks locally on this PC.", "เริ่มเซสชันบันทึกใหม่เพื่อจับเสียงระบบและ\nไมโครโฟนเก็บไว้ในเครื่องนี้"),
    ("Start with Windows", "เริ่มพร้อม Windows"),
    ("Status", "สถานะ"),
    ("Stereo mixed", "สเตอริโอรวม"),
    ("Stop", "หยุด"),
    ("Storage", "พื้นที่จัดเก็บ"),
    ("Storage & SQLite", "พื้นที่จัดเก็บและ SQLite"),
    ("Streaming chunk window", "ช่วงเวลาสตรีมต่อชิ้น"),
    ("Switch Light / Dark theme", "สลับธีมสว่าง / มืด"),
    ("System Audio", "เสียงระบบ"),
    ("System theme", "ธีมระบบ"),
    ("Temporary cache", "แคชชั่วคราว"),
    ("Test microphone", "ทดสอบไมโครโฟน"),
    ("Test system audio", "ทดสอบเสียงระบบ"),
    ("The licenses viewer", "ตัวแสดงไลเซนส์"),
    ("This action cannot be undone.", "ไม่สามารถย้อนกลับได้"),
    ("Toggle auto-scroll", "เปิด/ปิดเลื่อนอัตโนมัติ"),
    ("Track: System + Mic Mix", "แทร็ก: เสียงระบบ + ไมค์ผสม"),
    ("Transcribe", "ถอดเสียง"),
    ("Transcribed", "ถอดเสียงแล้ว"),
    ("Transcript copied to clipboard", "คัดลอกข้อความถอดเสียงแล้ว"),
    ("Transcript Detail", "รายละเอียดข้อความถอดเสียง"),
    ("Transcript directory", "โฟลเดอร์เก็บข้อความถอดเสียง"),
    ("Transcript title", "ชื่อข้อความถอดเสียง"),
    ("Transcription", "ถอดเสียง"),
    ("Transcription (OpenThai ASR)", "ถอดเสียง (OpenThai ASR)"),
    ("Transcripts", "ข้อความถอดเสียง"),
    ("UI Framework", "เฟรมเวิร์ก UI"),
    ("Verify checksum", "ตรวจสอบ checksum"),
    ("Checksum verification", "การตรวจสอบ checksum"),
    ("Version 0.4.2 · Windows 11 x86_64", "เวอร์ชัน 0.4.2 · Windows 11 x86_64"),
    ("Weekly", "รายสัปดาห์"),
    ("Win 11", "Win 11"),
    ("Words", "จำนวนคำ"),
    ("Today", "วันนี้"),
    ("WASAPI active", "WASAPI ทำงานอยู่"),
    ("Windows default", "ค่าเริ่มต้นของ Windows"),
    ("Starting", "กำลังเริ่ม"),
    ("Saved", "บันทึกแล้ว"),
    ("Resume", "ต่อ"),
    ("Ready", "พร้อม"),
    ("Off", "ปิด"),
    ("All system audio", "เสียงทั้งระบบ"),
    ("Waiting for the meeting app", "รอแอปประชุม"),
    ("Device lost", "อุปกรณ์หลุด"),
    ("Could not start", "เริ่มไม่ได้"),
    ("Recording problem", "การบันทึกมีปัญหา"),
    ("Recording saved to", "บันทึกไฟล์แล้วที่"),
    ("Turn on at least one audio source", "เปิดแหล่งเสียงอย่างน้อยหนึ่งแหล่ง"),
    ("No audio source could be started", "เริ่มแหล่งเสียงไม่ได้สักแหล่ง"),
    ("A recording is already running", "กำลังบันทึกอยู่แล้ว"),
    ("WASAPI device disconnected", "อุปกรณ์ WASAPI หลุดการเชื่อมต่อ"),
    ("ASR Engine", "เอนจินถอดเสียง"),
    ("ASR engine", "เอนจินถอดเสียง"),
    ("Dark theme", "ธีมมืด"),
    ("Default", "ค่าเริ่มต้น"),
    ("Database optimize is not wired up yet (UI preview)", "การปรับแต่งฐานข้อมูลยังไม่เชื่อมต่อ (ตัวอย่าง UI)"),
    ("Cache clearing is not wired up yet (UI preview)", "การล้างแคชยังไม่เชื่อมต่อ (ตัวอย่าง UI)"),
    ("Meeting date", "วันที่ประชุม"),
    ("SQLite database", "ฐานข้อมูล SQLite"),
    ("Show recordings list", "แสดงรายการบันทึก"),
    ("ONNX Runtime (CPU)", "ONNX Runtime (CPU)"),
    ("Thai (TH)", "ไทย (TH)"),
    ("Delete recording", "ลบการบันทึก"),
    ("Latency", "ความหน่วง"),
    ("Not available in this release. Planned for a future update.", "ยังไม่มีในรุ่นนี้ วางแผนไว้ในอัปเดตถัดไป"),
    ("Keyboard shortcuts", "คีย์ลัด"),
    ("Export", "ส่งออก"),
    ("free", "ว่าง"),
    ("48 kHz · 2-ch", "48 kHz · 2 ช่อง"),
    ("OpenThai ASR · ONNX Ready", "OpenThai ASR · ONNX พร้อม"),
    ("OpenThai ASR · Thai", "OpenThai ASR · ไทย"),
    ("5 finalized segments · 1 partial · 142 Thai words", "5 ส่วนที่เสร็จแล้ว · 1 บางส่วน · 142 คำไทย"),
    ("Latency: 210 ms · CPU: 14% (6 threads)", "ความหน่วง: 210 ms · CPU: 14% (6 เธรด)"),
    ("saved transcripts · Thai (OpenThai ASR)", "ข้อความถอดเสียงที่บันทึก · ไทย (OpenThai ASR)"),
    (" words", " คำ"),
    ("This will permanently delete ", "การลบนี้จะลบ "),
    (" and its dual-track WAV file from disk. This action cannot be undone.", " และไฟล์ WAV สองแทร็กออกจากดิสก์อย่างถาวร ไม่สามารถย้อนกลับได้"),
    ("Mono downmix", "โมโนรวม"),
    ("OpenThai ASR · Loading model...", "OpenThai ASR · กำลังโหลดโมเดล..."),
    ("OpenThai ASR · Unavailable", "OpenThai ASR · ใช้งานไม่ได้"),
    ("Loading model...", "กำลังโหลดโมเดล..."),
    ("Engine unavailable", "เอนจินใช้งานไม่ได้"),
    ("Transcription engine unavailable. Audio is still recorded; transcribe it later from Recordings.", "เอนจินถอดเสียงใช้งานไม่ได้ ระบบยังบันทึกเสียงอยู่ สามารถถอดเสียงภายหลังจากหน้าการบันทึก"),
    ("48,000 Hz matches Windows WASAPI shared mode and resamples cleanly for OpenThai ASR.", "48,000 Hz ตรงกับโหมดแชร์ของ Windows WASAPI และแปลงอัตราสุ่มเป็น OpenThai ASR ได้เรียบร้อย"),
    ("128.4 MB used · Safe to clear when idle", "ใช้ไป 128.4 MB · ล้างได้อย่างปลอดภัยเมื่อไม่ได้ใช้งาน"),
    ("Missing", "ไม่มี"),
    ("Downloading", "กำลังดาวน์โหลด"),
    ("Download model", "ดาวน์โหลดโมเดล"),
    ("Delete model", "ลบโมเดล"),
    ("Model folder", "โฟลเดอร์โมเดล"),
    ("Precision", "ชนิดน้ำหนักโมเดล"),
    ("Model", "โมเดล"),
    ("Model weights. fp32 is larger and slower; int8 is the default.", "น้ำหนักของโมเดล fp32 ใหญ่และช้ากว่า ค่าเริ่มต้นคือ int8"),
    ("Downloaded once, then kept on this computer.", "ดาวน์โหลดครั้งเดียว แล้วเก็บไว้ในเครื่องนี้"),
    ("Model is not installed", "ยังไม่ได้ติดตั้งโมเดล"),
    ("Model downloaded", "ดาวน์โหลดโมเดลแล้ว"),
    ("Model deleted", "ลบโมเดลแล้ว"),
    ("Download failed", "ดาวน์โหลดไม่สำเร็จ"),
    ("Not downloaded", "ยังไม่ได้ดาวน์โหลด"),
    ("Verifying model...", "กำลังตรวจสอบโมเดล..."),
    ("Ready", "พร้อม"),
    ("int8 (default, about 1 GB)", "int8 (ค่าเริ่มต้น ประมาณ 1 GB)"),
    ("fp32 (about 2.6 GB)", "fp32 (ประมาณ 2.6 GB)"),
    ("6 threads", "6 เธรด"),
    ("OpenThai ASR · Model not installed", "OpenThai ASR · ยังไม่ได้ติดตั้งโมเดล"),
    ("OpenThai ASR · Downloading model...", "OpenThai ASR · กำลังดาวน์โหลดโมเดล..."),
    ("Model not installed", "ยังไม่ได้ติดตั้งโมเดล"),
    ("Downloading model...", "กำลังดาวน์โหลดโมเดล..."),
    ("Partial transcription and the chunk window apply to live transcription, which comes later.", "ถอดเสียงบางส่วนและช่วงเวลาสตรีมใช้กับการถอดเสียงสด ซึ่งจะมาในภายหลัง"),
    ("Transcript", "ข้อความถอดเสียง"),
    ("The transcript appears after you stop recording", "ข้อความถอดเสียงจะปรากฏหลังคุณหยุดบันทึก"),
    ("No speech found", "ไม่พบเสียงพูด"),
    ("Transcribing...", "กำลังถอดเสียง..."),
    ("Transcribing", "กำลังถอดเสียง"),
    ("Waiting for the model", "รอโมเดล"),
    ("Waiting for the model. The recording is saved; transcribe it after the model is downloaded.", "รอโมเดล บันทึกเสียงถูกเก็บไว้แล้ว ถอดเสียงได้หลังดาวน์โหลดโมเดล"),
    ("Transcription failed", "ถอดเสียงไม่สำเร็จ"),
    ("Transcription cancelled", "ยกเลิกการถอดเสียงแล้ว"),
    ("Open Transcription settings", "เปิดการตั้งค่าถอดเสียง"),
    ("You", "คุณ"),
    ("Others", "คนอื่น"),
    ("lines", "บรรทัด"),
    ("Loading transcripts...", "กำลังโหลดทรานสคริปต์..."),
    ("Search transcripts or Thai text...", "ค้นหาทรานสคริปต์หรือข้อความภาษาไทย..."),
    ("Lines", "บรรทัด"),
    ("Could not open the transcript.", "เปิดทรานสคริปต์ไม่ได้"),
    ("Exported. The file is in the recording's folder.", "ส่งออกแล้ว ไฟล์อยู่ในโฟลเดอร์ของการบันทึก"),
    ("Could not export the transcript.", "ส่งออกทรานสคริปต์ไม่ได้"),
    ("Loading recordings...", "กำลังโหลดรายการบันทึก..."),
    ("Stop the recording before deleting.", "หยุดการบันทึกก่อนจึงจะลบได้"),
    ("This recording is being transcribed.", "รายการบันทึกนี้กำลังถอดเสียงอยู่"),
    ("Licences of the libraries and of the speech model: THIRD_PARTY_NOTICES.md in the project folder.", "สัญญาอนุญาตของไลบรารีและโมเดลเสียง: ไฟล์ THIRD_PARTY_NOTICES.md ในโฟลเดอร์โปรเจกต์"),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transcription_texts_have_thai() {
        for key in [
            "Missing",
            "Downloading",
            "Download model",
            "Delete model",
            "Model folder",
            "Precision",
            "Open folder",
            "Model is not installed",
            "Model downloaded",
            "Model deleted",
            "Download failed",
            "Not downloaded",
            "Verifying model...",
            "Ready",
            "int8 (default, about 1 GB)",
            "fp32 (about 2.6 GB)",
            "Model",
            "Model weights. fp32 is larger and slower; int8 is the default.",
            "Downloaded once, then kept on this computer.",
            "6 threads",
            "OpenThai ASR · Model not installed",
            "OpenThai ASR · Downloading model...",
            "Model not installed",
            "Downloading model...",
            "Partial transcription and the chunk window apply to live transcription, which comes later.",
        ] {
            assert!(has_thai(key), "missing Thai for {key:?}");
        }
    }

    #[test]
    fn recorder_transcription_texts_have_thai() {
        for key in [
            "Transcript",
            "The transcript appears after you stop recording",
            "No speech found",
            "Transcribing...",
            "Transcribing",
            "Waiting for the model",
            "Waiting for the model. The recording is saved; transcribe it after the model is downloaded.",
            "Transcription failed",
            "Transcription cancelled",
            "Open Transcription settings",
            "You",
            "Others",
            "lines",
            "Loading transcripts...",
            "Search transcripts or Thai text...",
            "Lines",
            "Could not open the transcript.",
            "Exported. The file is in the recording's folder.",
            "Could not export the transcript.",
            "Loading recordings...",
            "Stop the recording before deleting.",
            "This recording is being transcribed.",
            "Licences of the libraries and of the speech model: THIRD_PARTY_NOTICES.md in the project folder.",
        ] {
            assert!(has_thai(key), "missing Thai for {key:?}");
        }
    }

    /// One test only: the language is process-global, so parallel tests would race.
    #[test]
    fn translation_and_language_mapping() {
        assert_eq!(lang_from_index(0), Lang::En);
        assert_eq!(lang_from_index(1), Lang::Th);

        set_lang(Lang::En);
        assert_eq!(tr("Settings"), "Settings");

        set_lang(Lang::Th);
        assert_eq!(tr("Settings"), "ตั้งค่า");
        assert_eq!(
            tr("Speakers (Realtek(R) Audio)"),
            "Speakers (Realtek(R) Audio)"
        );
        assert!(not_wired("Checksum verification").contains("ยังไม่เชื่อมต่อ"));
        assert_eq!(tr("Windows default"), "ค่าเริ่มต้นของ Windows");

        set_lang(Lang::En);
    }
}
