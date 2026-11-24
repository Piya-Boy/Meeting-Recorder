//! Finding the meeting app (Zoom, Teams) whose audio to capture.

use std::collections::{HashMap, HashSet};

pub const MEETING_EXES: [&str; 3] = ["Zoom.exe", "ms-teams.exe", "Teams.exe"];

#[derive(Clone, Debug)]
pub struct ProcInfo {
    pub pid: u32,
    pub parent_pid: u32,
    pub exe: String,
}

fn is_meeting_exe(exe: &str) -> bool {
    MEETING_EXES.iter().any(|m| m.eq_ignore_ascii_case(exe))
}

/// Picks the process to capture: the root of a meeting app's process tree (a matching process
/// whose parent is not itself a match). With several roots, the first (lowest pid) whose tree
/// holds a pid in `active_audio` wins, else the lowest pid. `None` when nothing matches.
pub fn pick_target(procs: &[ProcInfo], active_audio: &HashSet<u32>) -> Option<u32> {
    let matches: HashSet<u32> = procs
        .iter()
        .filter(|p| is_meeting_exe(&p.exe))
        .map(|p| p.pid)
        .collect();
    let mut roots: Vec<u32> = procs
        .iter()
        .filter(|p| matches.contains(&p.pid) && !matches.contains(&p.parent_pid))
        .map(|p| p.pid)
        .collect();
    if roots.is_empty() {
        // Only a parent loop among matches; still capture something.
        roots = matches.iter().copied().collect();
    }
    roots.sort_unstable();
    roots.dedup();
    if roots.len() <= 1 {
        return roots.first().copied();
    }
    let parent: HashMap<u32, u32> = procs.iter().map(|p| (p.pid, p.parent_pid)).collect();
    let in_tree = |root: u32, mut pid: u32| {
        for _ in 0..=procs.len() {
            if pid == root {
                return true;
            }
            match parent.get(&pid) {
                Some(&next) if next != pid => pid = next,
                _ => return false,
            }
        }
        false
    };
    roots
        .iter()
        .copied()
        .find(|&r| active_audio.iter().any(|&a| in_tree(r, a)))
        .or_else(|| roots.first().copied())
}

/// Snapshot of the running processes; empty if the snapshot fails.
#[cfg(windows)]
pub fn list_processes() -> Vec<ProcInfo> {
    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
        TH32CS_SNAPPROCESS,
    };
    let mut out = Vec::new();
    // SAFETY: standard Toolhelp enumeration; the entry's dwSize is set before use and the
    // snapshot handle is closed on every path.
    unsafe {
        let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snap == INVALID_HANDLE_VALUE {
            return out;
        }
        let mut entry: PROCESSENTRY32W = std::mem::zeroed();
        entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
        let mut ok = Process32FirstW(snap, &mut entry);
        while ok != 0 {
            let len = entry
                .szExeFile
                .iter()
                .position(|&c| c == 0)
                .unwrap_or(entry.szExeFile.len());
            out.push(ProcInfo {
                pid: entry.th32ProcessID,
                parent_pid: entry.th32ParentProcessID,
                exe: String::from_utf16_lossy(&entry.szExeFile[..len]),
            });
            ok = Process32NextW(snap, &mut entry);
        }
        CloseHandle(snap);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    fn pi(pid: u32, parent_pid: u32, exe: &str) -> ProcInfo {
        ProcInfo {
            pid,
            parent_pid,
            exe: exe.to_string(),
        }
    }
    fn none() -> HashSet<u32> {
        HashSet::new()
    }

    #[test]
    fn none_when_no_match() {
        let procs = [pi(1, 0, "System"), pi(2, 1, "chrome.exe")];
        assert_eq!(pick_target(&procs, &none()), None);
    }

    #[test]
    fn single_root_is_returned_not_child() {
        let procs = [
            pi(100, 1, "Zoom.exe"),
            pi(101, 100, "CptHost.exe"),
            pi(102, 100, "Zoom.exe"),
        ];
        assert_eq!(pick_target(&procs, &none()), Some(100));
    }

    #[test]
    fn matching_is_case_insensitive() {
        assert_eq!(pick_target(&[pi(7, 1, "ZOOM.EXE")], &none()), Some(7));
        assert_eq!(pick_target(&[pi(8, 1, "Ms-Teams.exe")], &none()), Some(8));
    }

    #[test]
    fn teams_with_webview_children() {
        let procs = [
            pi(200, 1, "ms-teams.exe"),
            pi(201, 200, "msedgewebview2.exe"),
            pi(202, 200, "msedgewebview2.exe"),
        ];
        assert_eq!(pick_target(&procs, &none()), Some(200));
    }

    #[test]
    fn prefers_root_with_active_audio() {
        let procs = [
            pi(100, 1, "Teams.exe"),
            pi(200, 1, "Zoom.exe"),
            pi(250, 200, "CptHost.exe"),
        ];
        let active: HashSet<u32> = [250].into();
        assert_eq!(pick_target(&procs, &active), Some(200));
    }

    #[test]
    fn lowest_pid_when_none_active() {
        let procs = [pi(200, 1, "Zoom.exe"), pi(100, 1, "Teams.exe")];
        assert_eq!(pick_target(&procs, &none()), Some(100));
    }

    #[test]
    fn parent_loop_does_not_hang() {
        let procs = [
            pi(1, 2, "Zoom.exe"),
            pi(2, 1, "Zoom.exe"),
            pi(3, 3, "Teams.exe"),
        ];
        let active: HashSet<u32> = [9].into();
        assert!(pick_target(&procs, &active).is_some());
    }

    #[cfg(windows)]
    #[test]
    fn list_processes_finds_this_process() {
        let me = std::process::id();
        let procs = list_processes();
        let mine = procs
            .iter()
            .find(|p| p.pid == me)
            .expect("own pid in snapshot");
        assert!(
            mine.exe.to_lowercase().ends_with(".exe"),
            "exe was {:?}",
            mine.exe
        );
    }
}
