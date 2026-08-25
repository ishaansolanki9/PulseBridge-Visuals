# Development notes

## Runtime boundaries

The browser route (`/?performance=1`) renders quiet ambient motion with zero beat, onset, impact, and audio reactivity. It exists only to review shader appearance and must remain text-free. It is not an audio test path.

The Windows Tauri application owns the real path: Windows audio → bounded PCM ring → Rust analysis → latest visual snapshot → native `wgpu` renderer. React only reads status and writes user settings.

Run the browser UI:

```bash
npm run dev
```

Run the desktop shell:

```bash
npm run tauri -- dev
```

The configured control window is 960×720 with an 820×640 minimum. Native settings are stored as `visual-settings.json` in the platform application-config directory. Raw audio is never included in that file.

## Windows audio work

Open Rekordbox before PulseBridge to expose `Rekordbox (Detected)`. Process capture follows the Rekordbox process tree. If activation fails, the capture worker attempts the default Windows render endpoint; the user may also select any active render endpoint directly.

The Microsoft process-loopback API requires a sufficiently recent Windows build. Always verify the device-output fallback on the target party laptop. Native Windows work should be checked with the same commands used by `scripts/build-windows.ps1`.

## Visual work

The GLSL browser shader is the quiet appearance preview. The WGSL shader is the production renderer and must validate through the `native_performance_shader_is_valid_wgsl` test. Keep the five style shapes, palette values, and high-level settings aligned, but never feed generated rhythm into the browser to make it look reactive.

Auto direction is native because it depends on musical state. Manual styles and fixed palettes can still be reviewed in the browser. White flashes default to Off; turning flashes off must preserve colored motion impacts.

## Release

The NSIS bundle is enabled in `src-tauri/tauri.conf.json`. A clean Windows release command is:

```powershell
npm run tauri -- build --bundles nsis
```

See `WINDOWS_TRANSFER.md` for the first real-audio checklist and unsigned-development-installer warning.
