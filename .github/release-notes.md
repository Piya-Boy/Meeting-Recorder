## Before you run it

- Windows 10 or 11, 64-bit. This build has not been tried on a clean computer.
- The app needs the Microsoft Visual C++ Redistributable 2015-2022 (x64). Many computers already have it. If Windows reports that `MSVCP140.dll` or `VCRUNTIME140.dll` is missing, install it from Microsoft: https://aka.ms/vs/17/release/vc_redist.x64.exe
- The program is not code-signed, so Windows SmartScreen may warn on the first run. Check the download first (below), then choose **More info** and **Run anyway**.
- Transcription needs its model (about 1 GB at the default precision). Download it once from **Settings > Transcription**.

## Check the download

In PowerShell, in the folder that holds the zip, compare the result with the line in `SHA256SUMS.txt`:

```powershell
Get-FileHash .\meeting-recorder-*-windows-x86_64.zip -Algorithm SHA256
```

## Not working yet

These controls are on screen but do nothing yet:

- **Minimize to system tray** (shown as on): closing the window quits the app, which ends a recording in progress. Keep the window open while you record.
- **Start with Windows**.
- **Channel configuration**: the two sources are always saved as separate files.
- **Test system audio** and **Test microphone**.
- **Open-source licenses** button: the notices are in `THIRD_PARTY_NOTICES.md` inside the zip.

Keyboard shortcuts work only while the app window has focus.

## Licence

[PolyForm Noncommercial License 1.0.0](https://polyformproject.org/licenses/noncommercial/1.0.0): free for noncommercial use. Commercial use needs a separate licence from the copyright holder. See `LICENSE` inside the zip.
