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

## Changed since 0.0.1

- A source switched off in **Settings > Audio** is no longer recorded. In 0.0.1 it was.
- The system track is now everything that plays on the output device you choose under **System Audio**. 0.0.1 recorded Teams or Zoom alone when one was open, and the track could come out silent. Sounds from other programs, such as music and notifications, are now recorded too.
- The two source switches, the sample rate and the live-transcript settings are kept after a restart. In 0.0.1 a microphone you switched off was on again.
- If the output device you chose is not connected, when you press Record or during a recording, the system track records the Windows default and the Recorder says "Using Windows default". It goes back when your device returns. In 0.0.1 a chosen device that dropped stayed silent until it returned. A microphone is never swapped for another one.
- Thai tone marks that were drawn on top of an upper vowel, as in "ทั่วไป", are now drawn above it. Thai transcript text has taller lines to make room.
- Closing the window during a recording asks first.
- Settings that did nothing are gone, among them "Minimize to system tray" and "Start with Windows".
- The licence texts of the libraries, fonts and speech model are in the zip and in **Settings > Advanced > Open-source licenses**.
- The System theme follows Windows.

## Known limits

- Keyboard shortcuts work only while the app window has focus.
- The switch to the Windows default has not been tried with real hardware. If you change the Windows default while the old device stays connected, the recording does not follow it.
- Not tried yet on a real call with another person speaking. Choose the device you hear the meeting on, and check that the **SYS** meter moves while others speak. Sound that a meeting app plays on another device is not recorded.
- Outside the transcripts, Thai text that wraps keeps the old look from its second line on: a tone mark there can still sit on its vowel.
- Marks over the tall consonants ป, ฝ and ฟ are not moved aside for the tall stroke.

## Licence

[PolyForm Noncommercial License 1.0.0](https://polyformproject.org/licenses/noncommercial/1.0.0): free for noncommercial use. Commercial use needs a separate licence from the copyright holder. See `LICENSE` inside the zip.
