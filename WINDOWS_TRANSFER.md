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
5. Choose **Rekordbox detected (Safe Windows-output capture)** for the stable default, or choose the exact Windows output carrying PC MASTER OUT. Diagnostics still distinguish process detection, the selected capture route, and samples flowing. Process-specific capture remains disabled in normal builds until it passes the affected-laptop crash matrix.
6. Before fullscreen, run **Test connection → Audio only**, then **Renderer only**, then **Full startup**. Copy the JSON for each mode and record its report ID.
7. Leave **White flash** set to **Off** for the safest first run.
8. Press **Start live visuals**, then play your own file in Rekordbox.

Expected behavior:

- The TV is only abstract color and motion—no controls, text, waveform, or error messages.
- Phrase source initially reads **Audio inferred** (not Rekordbox phrase data). Bass/transients animate the active scene; inferred intros/verses/builds/choruses/breakdowns direct larger coherent scene changes.
- If audio stops or the device changes, the TV fades to ambient motion while the laptop window reports recovery. It does not replay old reactions.
- **Ambient**, **Black screen**, and **Stop visuals** remain available from the laptop window.
- Focused `Escape`, the normal Windows close shortcut, and emergency `Ctrl+Alt+Shift+F12` stop only the visual session and return focus to the controller.

Process-specific loopback is a Windows API available on newer Windows builds, but it is not used by the normal build because the affected laptop repeatedly terminated with native heap corruption before its asynchronous activation callback returned. The Rekordbox-aware selection now uses stable default-output loopback. Selecting the exact output device directly remains the most compatible route.

If Start fails, PulseBridge must remain open and display a concise error, **Retry live visuals**, the copyable report, and **Open logs folder**. Record the report/session ID, error code, selected adapter/backend, Windows build, Rekordbox version, and route. The code changes alone do not confirm the reported Windows termination is fixed; the affected laptop's reports/Event Viewer evidence must identify the failing stage.

## Real-party validation checklist

Because no synthetic song is bundled, do these checks with your real files:

- Test at least one quiet intro, build/drop, high-energy chorus, breakdown, abrupt track change, and silence.
- Confirm the selected HDMI display at 1080p; then test 4K if that is the party setup.
- Stop/start Rekordbox playback and change the selected Windows output once.
- Disconnect/reconnect HDMI before the event and verify PulseBridge can be stopped and restarted cleanly.
- Leave a representative playlist running for the full expected party duration and watch for memory growth, accumulated lag, or a frozen output.
- Start/Stop 20 times, including rapid repeat clicks; force a bad display, process-capture failure, silence, Rekordbox exit/restart, output-device change, and HDMI disconnect/reconnect.
- Check 1080p and 4K at 16:9 plus at least one 16:10/4:3/ultrawide surface. Confirm no negative-X center seam, desktop, labels, debug text, white startup frame, or old impact replay.
- Exercise quiet intros, verses, builds, choruses/drops, breakdowns, bridges, outros, loops, seeks, pause, deck changes, and abrupt track transitions. Confirm macro changes stay coherent and Auto never piles up more than two families.

The app never records or writes captured audio to disk. Its PCM and feature histories are bounded and kept only in memory.

## If the visuals do not react

- Confirm a track is visibly playing in Rekordbox and click the audio-source refresh button in PulseBridge.
- Try **Rekordbox detected (Safe Windows-output capture)** and verify that **Samples flowing** appears; otherwise choose the exact Windows output carrying the master audio.
- If a DJ controller is using an ASIO path that bypasses normal Windows audio, open Rekordbox **Preferences → Audio** and enable **Output audio from the computer's built-in speakers and your DJ equipment (PC MASTER OUT)** when your controller supports it. Then select that computer output in PulseBridge.
- Confirm Windows Volume Mixer shows Rekordbox producing audio and that PulseBridge is not in Ambient or Black screen mode.
- If the wrong monitor opens, stop visuals, use Windows **Settings → System → Display → Identify**, then select the numbered TV and start again.
