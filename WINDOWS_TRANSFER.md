# Windows transfer and first real-audio test

PulseBridge intentionally contains no synthetic song or reactive test feed. Its first end-to-end audio test happens on the Windows DJ laptop with audio actually playing from Rekordbox.

## What to transfer

Use one of these routes:

1. **Build on the Windows laptop:** copy `transfer-ready/PulseBridge-Windows-Source.zip`, extract it to a normal writable folder, and run the included build script.
2. **Build with GitHub Actions:** push this repository to GitHub, run the `PulseBridge CI` workflow, and download the `PulseBridge-Windows-Installer` artifact. This route produces the installer on a clean Windows runner.

The finished installer is all another Windows machine needs. Node, Rust, source code, and a terminal are not needed after it is installed.

## One-command Windows build

Open PowerShell in the extracted project folder and run:

```powershell
Set-ExecutionPolicy -Scope Process Bypass
.\scripts\build-windows.ps1
```

The script checks Windows, installs Node.js LTS, Rust, Microsoft C++ Build Tools, and the Windows SDK through `winget` if they are missing, installs locked dependencies, runs the frontend and Rust checks, and builds the NSIS installer. The C++ tools can trigger a Windows administrator prompt and take several minutes. If a newly installed tool is not visible immediately, restart Windows, reopen PowerShell in the project folder, and rerun the two commands.

Successful output is copied to:

```text
transfer-ready\PulseBridge Visuals Setup.exe
transfer-ready\PulseBridge Visuals Setup.sha256.txt
```

The current development installer is unsigned, so Windows SmartScreen may show an “unrecognized app” message. Verify the SHA-256 against the adjacent text file before choosing **More info → Run anyway**. A public release should be code-signed.

## First real test with Rekordbox

1. Connect the TV/projector by HDMI and set Windows to extend the desktop.
2. Confirm Windows and Rekordbox are sending music to the intended TV, interface, or speakers.
3. Open Rekordbox before PulseBridge so it is detected automatically.
4. Open PulseBridge Visuals and select the TV under **Output display**.
5. Choose **Rekordbox (Detected)** for process-only capture. If it is unavailable, choose the exact Windows output Rekordbox is using.
6. Leave **White flash** set to **Off** for the safest first run.
7. Press **Start live visuals**, then play your own file in Rekordbox.

Expected behavior:

- The TV is only abstract color and motion—no controls, text, waveform, or error messages.
- Bass changes displacement and expansion; transient hits produce pulses; builds increase tension; major impacts can trigger Burst behavior; quieter passages relax toward Fluid.
- If audio stops or the device changes, the TV fades to ambient motion while the laptop window reports recovery. It does not replay old reactions.
- **Ambient**, **Black screen**, and **Stop visuals** remain available from the laptop window.

Process-specific loopback is a Windows API available on newer Windows builds. PulseBridge automatically attempts default-output loopback if Rekordbox-only activation fails. Selecting an output device directly is the most compatible fallback.

## Real-party validation checklist

Because no synthetic song is bundled, do these checks with your real files:

- Test at least one quiet intro, build/drop, high-energy chorus, breakdown, abrupt track change, and silence.
- Confirm the selected HDMI display at 1080p; then test 4K if that is the party setup.
- Stop/start Rekordbox playback and change the selected Windows output once.
- Disconnect/reconnect HDMI before the event and verify PulseBridge can be stopped and restarted cleanly.
- Leave a representative playlist running for the full expected party duration and watch for memory growth, accumulated lag, or a frozen output.

The app never records or writes captured audio to disk. Its PCM and feature histories are bounded and kept only in memory.

## If the visuals do not react

- Confirm a track is visibly playing in Rekordbox and click the audio-source refresh button in PulseBridge.
- Try **Rekordbox (Detected)** first, then choose the exact Windows output that is carrying the master audio.
- If a DJ controller is using an ASIO path that bypasses normal Windows audio, open Rekordbox **Preferences → Audio** and enable **Output audio from the computer's built-in speakers and your DJ equipment (PC MASTER OUT)** when your controller supports it. Then select that computer output in PulseBridge.
- Confirm Windows Volume Mixer shows Rekordbox producing audio and that PulseBridge is not in Ambient or Black screen mode.
- If the wrong monitor opens, stop visuals, use Windows **Settings → System → Display → Identify**, then select the numbered TV and start again.
