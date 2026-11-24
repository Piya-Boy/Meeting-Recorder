# Update check - design

Date: 2026-11-25. Branch `feature/update-check` (from `main`).
Status: the user asked whether auto-update was done, chose "app auto-update", then chose option 1 of 3 (tell the user a newer version exists, do not install it) together with a workflow that builds the releases. The calls below are the author's.

## Intent

A user running an old build should learn that a newer one exists without watching the repository. The app is one unsigned exe, so it does not replace itself: the user downloads the new build from the release page.

## Goals

- At start-up, and from Settings > Advanced, ask GitHub Releases whether a newer version exists and say so in the app.
- A switch in Settings > General to turn the automatic check off.
- A GitHub Actions workflow that builds a Windows zip with a SHA-256 file and publishes a Release when a version tag is pushed.

## Non-goals

- Downloading or installing the update, signature checks, code signing, delta updates, release notes inside the app, a changelog screen.
- A build-and-test workflow for pull requests.

## Constraints

- Local-first app: the check sends no recording or personal data. GitHub sees the IP address and a `meeting-recorder/<version>` User-Agent, and the switch says the app asks GitHub.
- The answer from the network is untrusted: the version must look like `[v]N.N.N` (pre-releases, drafts and odd tags are ignored), and the link the app opens is built by the app from that version, never taken from the answer.
- A failed check never interrupts the app; it is only reported when the user asked for it.
- The repository has no release yet; GitHub answers 404 then, which means "up to date".

## Approaches

1. Notify only (chosen): small, reuses `ureq`, nothing from the network is executed.
2. Download, verify SHA-256, replace the exe on the next start: convenient, but a SHA-256 file in the same release does not protect against a hijacked account; it needs an ed25519 signature and a key inside the app.
3. Same as 2 with a confirmation: still needs signing.
Approach 2 or 3 can grow from this later.

## Design

- `update` module: `VERSION` (from `Cargo.toml`, replacing the four hard-coded "0.4.2" texts), `parse_version`, `is_newer`, `release_from_json`, `check(api, current)` (GET `{api}/repos/Piya-Boy/Meeting-Recorder/releases/latest`, 15 s limit, 256 KiB body limit, 404 = up to date), `spawn` (one thread, one answer on a channel) and `due(enabled, last_check, now)`.
- The check runs at start-up when the switch is on and the last good check is a day old or more (`update.last_check` in `settings.ini`, written after an answer from GitHub, including 404), once per run. Dev-flag runs and tests do not check; `--update-url=<base>` makes a dev run check that server at start-up.
- `AppState` keeps `UpdateState` (Idle, Checking, UpToDate, Available, Failed). An automatic check says something only when a version is found (a toast, and a link in the sidebar footer); a check from Settings > Advanced always answers with a toast and a line under the version.
- Settings: General gets "Check for updates" (default on, saved as `update.check`). Advanced > About gets a "Check for updates" button, a status line and, when a version is found, "Open download page" (opens the built link in the default browser).
- Workflow `.github/workflows/release.yml`: on a tag `vN.N.N` (must equal the `Cargo.toml` version) or by hand; builds on `windows-latest` with `cargo test --locked` and `cargo build --release --locked`, zips the exe with `LICENSE`, `THIRD_PARTY_NOTICES.md` and `README.md`, writes `SHA256SUMS.txt`, uploads them as an artifact; a second job on a tag creates the Release with the `gh` CLI. Actions are pinned to commit SHAs. A hand-started run only builds the artifact, so it can be tried without publishing.

## Left to the builder

Wording, exact placement, spacing and colours; the collapsed sidebar shows no update hint.
