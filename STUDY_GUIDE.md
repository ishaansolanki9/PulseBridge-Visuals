# PulseBridge Visuals: complete project study guide

This guide is written for someone starting from zero. It explains the vocabulary, Rust, TypeScript/React, Tauri, audio analysis, GPU rendering, concurrency, reliability decisions, testing, and the interview-level reasoning behind this repository.

It describes the current working project, not an imaginary future version. Where the product has an important limitation, the limitation is stated plainly.

## How to use this guide

Do not try to memorize the repository file by file on the first pass. Learn it in layers:

1. Read **The project in one page** until you can explain the product without code.
2. Learn the terms in **Foundations and vocabulary**.
3. Work through **Rust from zero** and run the small examples mentally.
4. Trace **A complete live session** from the Start button to a GPU frame.
5. Study audio, analysis, phrase inference, scene direction, and rendering in that order.
6. Read **Reliability and diagnostics** because this is where many of the strongest engineering decisions live.
7. Practice the interview questions aloud. Aim to explain the reasoning, not recite wording.
8. Use the exercises at the end to prove to yourself that you understand the code.

Useful project references:

- [README.md](README.md) — product overview and primary commands
- [docs/architecture.md](docs/architecture.md) — concise architecture summary
- [docs/audio-analysis.md](docs/audio-analysis.md) — capture and signal-analysis summary
- [docs/reliability.md](docs/reliability.md) — failure and cleanup model
- [docs/visual-language.md](docs/visual-language.md) — scene and color intent
- [docs/development.md](docs/development.md) — development boundaries and release notes
- [docs/rekordbox-phrase-integration.md](docs/rekordbox-phrase-integration.md) — the honest Rekordbox metadata boundary

---

## 1. The project in one page

### One-sentence explanation

PulseBridge is a cross-platform desktop application that captures live Rekordbox-related audio, converts it into bounded musical features, and renders full-screen, music-reactive GPU visuals on a second display while keeping control and diagnostics on the laptop.

### The problem it solves

A DJ may have music in Rekordbox but no lighting/video system. PulseBridge turns a TV, projector, or second monitor into an abstract light source. It reacts to loudness, frequency balance, onsets, beat-like pulses, energy trends, and inferred musical sections.

It is deliberately not:

- a music player;
- a waveform viewer;
- a DMX or physical-fixture controller;
- a cloud service;
- an audio recorder;
- a Rekordbox database scraper;
- a source of verified live Rekordbox phrase metadata.

### The most important truth about Rekordbox integration

The project does **not** currently receive phrase names, beat grids, playhead position, active deck, or track identity from a documented live Rekordbox API. It captures audio and infers longer musical structure from that audio. The UI labels the provenance **Audio inferred** so it does not pretend that inference came from Rekordbox metadata.

On Windows, the preferred route is Rekordbox process-loopback capture. If that route is unavailable, the automatic source can explicitly fall back to the default Windows output. A manually selected output route captures that output device. On macOS 14.2 or newer, the implementation uses a private Core Audio process tap for Rekordbox and does not claim an older-OS system-output fallback.

### Technology stack

| Layer | Technology | Responsibility |
| --- | --- | --- |
| Controller UI | React 19 + TypeScript | Settings, status, Start/Stop, safety controls, diagnostic reports |
| Web development tool | Vite | Development server and frontend bundle |
| Desktop shell/bridge | Tauri 2 | Hosts the controller and exposes Rust commands to TypeScript |
| Native application | Rust 2021 | Lifecycle, settings, audio capture, analysis, scene direction, diagnostics |
| Windows audio | WASAPI + Windows APIs | Process loopback and render-endpoint loopback |
| macOS audio | Core Audio + AppKit/Foundation FFI | Rekordbox process discovery and process tap capture |
| Audio analysis | `rustfft` + custom heuristics | FFT bands, spectral flux, onset, beat pulse, musical state |
| Thread handoff | `crossbeam_queue`, `Arc`, locks, atomics | Bounded PCM and latest-state sharing |
| Production rendering | `wgpu` + WGSL | Native full-screen GPU output |
| Browser preview | WebGL 2 + GLSL | Non-reactive appearance preview only |
| Serialization | Serde + JSON | Tauri payloads, settings, reports, structured logs |
| Testing/CI | Cargo, ESLint, TypeScript, Vite, GitHub Actions | Unit, static, bundle, and platform compile checks |

### The main pipeline

```text
Rekordbox process or supported output route
                  |
          platform capture worker
       (WASAPI or Core Audio tap)
                  |
     mono f32 samples at 48,000 Hz
                  |
      bounded overwriting PCM queue
                  |
          music-analysis worker
       FFT -> rhythm -> state -> phrase
                  |
         latest AnalysisSnapshot
         latest PlaybackContext
                  |
             SceneDirector
     base + crossfade + 0-2 modifiers
                  |
       native wgpu renderer worker
                  |
        raw full-screen Tauri Window

React controller --commands/settings/status--> Rust
```

The key separation is that the React controller is **not** in the real-time visual path. A slow or minimized webview cannot stall audio capture or GPU drawing.

### Three interview pitches

#### 30 seconds

> PulseBridge is a Tauri desktop app that turns live DJ audio into second-screen visuals. React is only the controller. Rust captures process or output audio, normalizes it to mono 48 kHz, computes FFT/rhythm features, infers musical state and phrases, directs a bounded scene plan, and sends uniforms to a native `wgpu` shader. The architecture uses bounded queues, latest-value snapshots, transactional startup, and explicit diagnostics so UI lag or audio loss cannot freeze the performance display.

#### 90 seconds

> The product has two intentionally separate visual paths. The browser path is a non-reactive WebGL appearance preview. The real desktop path is native: WASAPI on Windows or Core Audio process taps on macOS feed an overwriting lock-free PCM ring. A Rust worker analyzes 2048-sample Hann-windowed FFT frames on a 960-sample hop, producing normalized energy bands, spectral flux, onset strength, an onset-derived beat pulse, and a hysteretic musical state. A bounded longer-horizon provider infers phrase-like sections without claiming Rekordbox metadata. A scene director chooses one of eight base families, permits only a normalized two-base crossfade, and schedules at most two compatible modifiers. A `wgpu` worker reads only the latest snapshots and renders a full-screen triangle with WGSL. Startup keeps the window hidden until the first GPU frame is presented; failures roll back workers and leave the controller alive.

#### Five-minute structure

When asked for more detail, explain in this order:

1. Product and honest scope.
2. UI/native boundary through Tauri commands.
3. Platform audio capture and 48 kHz mono contract.
4. Bounded queue and analysis worker.
5. Musical state, inferred phrase context, and scene direction.
6. Native `wgpu` surface and shader uniforms.
7. Transactional lifecycle, ambient degradation, diagnostics, and tests.

---

## 2. Foundations and vocabulary

### Program, process, and thread

- A **program** is code stored on disk.
- A **process** is a running instance of a program with its own memory.
- A **thread** is one path of execution inside a process. Threads in the same process share memory.

PulseBridge is one process with several important execution contexts:

- the controller webview/JavaScript event loop;
- the Tauri/native application runtime;
- an audio-capture worker;
- an audio-analysis worker;
- a GPU-renderer worker;
- short-lived join-supervisor and diagnostic workers.

### Frontend and backend in this desktop app

“Frontend” here means the React controller displayed in a webview. “Backend” means native Rust code in the same desktop application, not a remote web server. TypeScript calls named Tauri commands; Tauri serializes arguments into Rust values and serializes results back into TypeScript-compatible JSON.

### Native code

Native code runs as machine code on the operating system and can use platform APIs. PulseBridge needs native code because browsers cannot directly create Windows WASAPI process-loopback streams, macOS Core Audio process taps, or the project’s raw production `wgpu` surface.

### Sampling and PCM

Digital audio is a sequence of numbers called **samples**. PulseBridge’s internal contract is:

- 48,000 samples per second;
- one channel (mono);
- `f32` floating-point values;
- values clamped to approximately `-1.0..1.0`.

PCM means pulse-code modulation: ordinary sampled audio values. At the default ten-second capacity, the ring can contain 480,000 `f32` samples, about 1.92 MB of sample values before queue overhead. This is capacity, not an intentional ten-second playback delay—the analysis worker normally drains current samples immediately.

### Buffer, queue, and ring buffer

A **buffer** temporarily holds data between components that run at different speeds. The project wraps `crossbeam_queue::ArrayQueue<f32>` as `PcmRingBuffer`.

Its policy is crucial: when full, it removes the oldest sample and inserts the newest. For a live light show, stale audio is less valuable than current audio. This prevents unbounded memory growth and avoids replaying an old drop after a stall.

### Snapshot

A snapshot is the latest complete state. The analysis worker replaces one `AnalysisSnapshot`; the renderer reads it. There is no queue of visual events to replay. This is a **latest-value** or **state-based** design rather than an event-log design.

### Serialization and JSON

Serialization converts in-memory values into a transport/storage format. Serde derives this logic for Rust structs and enums. `#[serde(rename_all = "camelCase")]` makes Rust’s `sample_flow` appear as TypeScript’s `sampleFlow`.

### GPU, shader, uniform, and surface

- A **GPU** performs massively parallel graphics computation.
- A **shader** is a small GPU program. PulseBridge’s production shader is WGSL.
- A **uniform** is a small read-only parameter block shared by shader invocations for one draw.
- A **surface** connects rendered GPU textures to an operating-system window.
- A **swapchain/presentation path** provides textures that are displayed frame by frame. Modern `wgpu` exposes this through surface configuration and current surface textures.

PulseBridge draws one oversized triangle covering the screen. The fragment shader runs for every output pixel and computes color procedurally. No image or geometry asset is needed.

### Soft real-time, not hard real-time

The application is latency-sensitive, but it is not a hard-real-time system. It uses normal OS threads, locks, sleeps, allocations, and GPU presentation. It aims to stay current and fail gracefully; it does not prove a strict worst-case deadline.

### FFI and `unsafe`

FFI means foreign-function interface: Rust calling APIs defined outside Rust, such as Windows COM, Core Audio C functions, Objective-C objects, or macOS keyboard state functions.

Rust marks operations it cannot statically prove safe with `unsafe`. `unsafe` does not mean “incorrect”; it means the programmer must uphold extra invariants such as valid pointers, correct lifetimes, matching formats, and exactly-once resource release. The safe Rust modules should contain and minimize these boundaries.

---

## 3. Repository map

### Root files

| Path | Purpose |
| --- | --- |
| `README.md` | Product scope, supported paths, development and verification commands |
| `package.json` | npm workspace root; forwards commands to `app` and exposes Tauri CLI |
| `package-lock.json` | Exact JavaScript dependency graph for reproducible `npm ci` |
| `WINDOWS_TRANSFER.md` | Moving source to and building on Windows |
| `CODEX_IMPLEMENTATION_BRIEF.md` | Implementation requirements/history; useful context, not runtime code |
| `REKORDBOX_CRASH_DIAGNOSTICS_VISUALS_V2_BRIEF.md` | Reliability and hardware-validation brief |
| `.github/workflows/ci.yml` | Frontend, Windows, and macOS CI jobs |
| `scripts/make-transfer-zip.sh` | Creates a small source archive while excluding build products and logs |
| `scripts/build-windows.ps1` | Installs prerequisites, checks, builds NSIS, copies installer and hash |

The existing ZIP archive is an artifact, not source code. Build directories such as `node_modules`, `app/dist`, and `src-tauri/target` are generated and should not be learned as authored architecture.

### Frontend: `app/`

| Path | Purpose |
| --- | --- |
| `app/index.html` | HTML entry containing the React root |
| `app/vite.config.ts` | Vite configuration |
| `app/tsconfig.json` | TypeScript compiler configuration |
| `app/eslint.config.js` | Lint rules |
| `app/src/main.tsx` | Mounts React in `StrictMode` |
| `app/src/App.tsx` | Selects controller vs `?performance` preview route |
| `app/src/control/ControlApp.tsx` | Controller state, polling, controls, reports, status labels |
| `app/src/control/transport.ts` | Browser vs Tauri implementations of one command interface |
| `app/src/visuals/types.ts` | TypeScript mirror of settings/status/report payloads |
| `app/src/visuals/model.ts` | Browser-preview style, palette, intensity, envelope helpers |
| `app/src/visuals/PerformanceCanvas.tsx` | WebGL 2 ambient canvas and animation loop |
| `app/src/visuals/PerformanceOutput.tsx` | Text-free browser performance-preview wrapper |
| `app/src/visuals/shader.ts` | GLSL preview shader |
| `app/src/styles.css` | Controller layout and visual styling |

### Native Rust: `src-tauri/`

| Path | Purpose |
| --- | --- |
| `Cargo.toml` / `Cargo.lock` | Rust crate metadata and locked dependencies |
| `tauri.conf.json` | App identity, controller window, CSP, bundle targets |
| `build.rs` | Runs Tauri build setup and records the compilation target |
| `Info.plist` | macOS metadata including audio-capture usage explanation |
| `src/main.rs` | Tiny binary entry that calls the library runtime |
| `src/lib.rs` | Builds Tauri, initializes state/logging, registers commands, cleans exit |
| `src/commands.rs` | Tauri command boundary used by TypeScript |
| `src/config/mod.rs` | Loads, sanitizes, and persists visual settings |
| `src/display/mod.rs` | `PerformanceManager`, session lifecycle, windows, worker supervision |
| `src/audio/mod.rs` | Platform-backend abstraction and public audio entry points |
| `src/audio/types.rs` | Audio source, route, lifecycle, flow, and metrics types |
| `src/audio/ring_buffer.rs` | Bounded overwrite-oldest 48 kHz PCM queue |
| `src/audio/format.rs` | Format validation, PCM decoding/downmixing, streaming resampling |
| `src/audio/reconnect.rs` | Route-specific silence/retry policy |
| `src/audio/wasapi.rs` | Windows process/output capture and COM/RAII management |
| `src/audio/coreaudio.rs` | macOS process discovery, tap, aggregate device, callback pipeline |
| `src/audio/platform_stub.rs` | Honest unsupported behavior on other desktop OSes |
| `src/analysis/fft.rs` | Hann FFT, bands, flux, energy envelopes, rhythm call |
| `src/analysis/normalization.rs` | Rolling percentile normalization |
| `src/analysis/rhythm.rs` | Onset-derived beat interval, phase, and pulse |
| `src/analysis/musical_state.rs` | Hysteretic musical-state and impact logic |
| `src/analysis/types.rs` | Feature, visual input, snapshot, and audio-freshness types |
| `src/analysis/mod.rs` | Analysis worker and phrase-provider update loop |
| `src/phrase/mod.rs` | Playback-context contract and audio-inferred phrase provider |
| `src/visuals/state.rs` | Settings, smoothing envelopes, intensity, flash safety |
| `src/visuals/palette.rs` | Four-color palettes and smooth interpolation |
| `src/visuals/director.rs` | Base-family selection, dwell, crossfades, modifiers, budgets |
| `src/visuals/renderer.rs` | Surface/device/pipeline creation, render loop, recovery, probe |
| `shaders/performance.wgsl` | Production full-screen shader |
| `src/diagnostics.rs` | Structured rotating logs, durable marker, report persistence |
| `src/diagnostic_runner.rs` | Bounded audio/GPU/hidden-surface diagnostic workflows |
| `src/resilience/power.rs` | Windows display/system-sleep guard using RAII |

Icons, generated Tauri schemas, and bundle metadata support packaging; they are not part of the runtime signal pipeline.

---

## 4. Rust from zero, using this project

### What Rust is

Rust is a compiled systems programming language. It aims to provide low-level control without garbage collection while preventing many memory and concurrency bugs at compile time.

In this project Rust is a good fit because it can:

- call Windows and macOS native APIs;
- manage GPU resources;
- run long-lived worker threads;
- enforce resource cleanup with ownership and `Drop`;
- share state with explicit synchronization;
- expose strongly typed commands through Tauri.

### Crate, package, module, and path

- A Cargo **package** is described by `Cargo.toml`.
- A **crate** is a Rust compilation unit. This package has a library crate and a small binary crate.
- A **module** organizes names inside a crate.
- A path such as `crate::audio::CaptureStatus` starts at the current crate root.

`src/main.rs` calls `pulsebridge_lib::run()`. The library’s `lib.rs` declares modules with `mod analysis;`, `mod audio;`, and so on. `pub` makes an item visible outside its module; `pub(crate)` limits it to the crate.

`Cargo.toml` asks Cargo to produce three library forms: `rlib` for ordinary Rust linking, `staticlib` for a static native library, and `cdylib` for a C-compatible dynamic library. Tauri uses these forms across desktop/mobile targets even though this project’s implemented product paths are desktop Windows and macOS. `build.rs` runs Tauri’s build integration and exposes the compilation target string to diagnostics.

### Important Rust dependencies

| Crate | Why it exists here |
| --- | --- |
| `tauri` / `tauri-build` | Desktop runtime, windows, commands, build metadata |
| `wgpu` | Cross-platform native GPU abstraction |
| `bytemuck` | Safe byte view of the plain uniform struct |
| `rustfft` | FFT planning and execution |
| `crossbeam-queue` | Bounded lock-free PCM queue |
| `serde` / `serde_json` | Tauri payloads, settings, reports, logs |
| `chrono` | RFC3339 diagnostic timestamps |
| `uuid` | Session/report/temp-file identifiers |
| `pollster` | Blocks the renderer thread on async `wgpu` setup |
| `windows` / `windows-core` | Typed Windows API and COM bindings |
| `coreaudio-sys`, `core-foundation`, `objc2`, `libc` | macOS Core Audio/Cocoa/Core Foundation FFI |
| `naga` (test only) | Parses and validates WGSL without launching the app |

### Variables, mutability, and basic types

Rust variables are immutable by default:

```rust
let sample_rate = 48_000;      // inferred integer type
let mut captured_frames = 0_u64;
captured_frames += 1;
```

Common types here:

- `bool` — true/false;
- `u8`, `u16`, `u32`, `u64`, `usize` — unsigned integers;
- `i32` — signed integer used by native APIs;
- `f32`, `f64` — floating point;
- `String` — owned UTF-8 string;
- `&str` — borrowed string slice;
- arrays such as `[f32; 4]` — fixed size;
- `Vec<T>` — growable contiguous collection;
- `VecDeque<T>` — double-ended queue;
- tuples such as `(MusicState, f32)`.

Numeric suffixes such as `0_u64` make the intended type explicit. Underscores improve readability and do not change a number.

### Structs

A struct groups named fields:

```rust
pub struct CaptureStatus {
    pub state: CaptureState,
    pub packets_received: bool,
    pub rms: f32,
}
```

An instance owns its fields. Methods are defined in an `impl` block. `Self` refers to the implementing type.

### Enums and state modeling

Rust enums are closed sets of variants:

```rust
pub enum OutputMode {
    Reactive,
    Ambient,
    Black,
}
```

They are safer than arbitrary strings because the compiler forces code to consider possible variants. This project uses enums for audio lifecycle, renderer lifecycle, musical state, diagnostic verdict, palette, visual family, and much more.

`match` handles variants:

```rust
let value = match mode {
    OutputMode::Reactive => 0,
    OutputMode::Ambient => 1,
    OutputMode::Black => 2,
};
```

### `Option<T>`: a value may be absent

Rust avoids null for ordinary safe code. `Option<T>` is either `Some(value)` or `None`.

Examples:

- `last_audio_at: Option<Instant>` — there may not have been any audio yet;
- `secondary: Option<VisualFamily>` — most scenes have no second base;
- `message: Option<String>` — status text is optional.

Useful methods in the repository include `.map(...)`, `.and_then(...)`, `.unwrap_or(...)`, `.unwrap_or_else(...)`, `.is_some_and(...)`, and `.map_or(...)`.

### `Result<T, E>`: an operation may fail

`Result<T, E>` is either `Ok(value)` or `Err(error)`. For example:

```rust
pub fn stop(&self) -> Result<(), String>
```

`()` is the unit type: success has no additional value. The `?` operator returns early on error and unwraps success:

```rust
let payload = serde_json::to_vec_pretty(settings)
    .map_err(|error| error.to_string())?;
```

This project uses `Result` for expected operational failures such as a missing display, unsupported audio format, GPU creation failure, or file error. It reserves panics for programming/runtime failures and installs panic diagnostics.

### Ownership

Every Rust value has an owner. When the owner goes out of scope, the value is dropped. Moving a non-`Copy` value transfers ownership:

```rust
let source_id = settings.audio_source_id.clone();
spawn_audio_capture(source_id, ring, stop, status);
```

The clone is intentional because the worker needs owned data that outlives the current function call.

Types such as small numeric enums can derive `Copy`, so assignment copies bits instead of moving ownership. Heap-owning `String`, `Vec`, and most resource handles are not implicitly copied.

### Borrowing and references

A reference borrows without taking ownership:

- `&T` — shared, read-only borrow;
- `&mut T` — exclusive mutable borrow.

For example, `convert_interleaved_to_mono_into(bytes, ..., output: &mut Vec<f32>)` reads borrowed bytes and reuses a caller-owned output vector. Reuse reduces repeated allocation.

Rust’s borrow checker prevents data from being mutated through two unsynchronized aliases. When threads truly need shared mutable state, the project uses synchronization types explicitly.

### Lifetimes

A lifetime describes how long a reference remains valid. Most lifetimes are inferred. `WasapiPacket<'a>` explicitly states that the packet borrows its `IAudioCaptureClient` and cannot outlive it.

The renderer’s `wgpu::Surface<'static>` deserves nuance: surface creation takes ownership of a cloned window handle in a way that lets `wgpu` retain the required target lifetime. The raw surface is still cleaned up with its Rust owner.

### Traits

A trait defines shared behavior. `PlatformCaptureBackend` says every platform backend must be able to enumerate sources and spawn capture:

```rust
trait PlatformCaptureBackend {
    fn enumerate_sources() -> Result<Vec<AudioSourceInfo>, String>;
    fn spawn_capture(/* ... */) -> Result<JoinHandle<()>, String>;
}
```

Windows, macOS, and the unsupported stub provide separate implementations. The rest of the application calls one platform alias instead of scattering OS checks across the code.

Other important traits:

- `Default` supplies safe initial values;
- `Serialize` and `Deserialize` support JSON/Tauri conversion;
- `Clone` explicitly duplicates a value;
- `Copy` permits bitwise copy semantics;
- `Drop` performs deterministic cleanup;
- `Send` means a value may move between threads;
- `Pod`/`Zeroable` from `bytemuck` prove the uniform struct can be represented as GPU bytes.

### Derive macros and attributes

Attributes begin with `#[]`:

```rust
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeSnapshot { /* ... */ }
```

Derive macros generate repetitive implementations. Conditional compilation attributes select platform code:

```rust
#[cfg(target_os = "windows")]
mod wasapi;
```

Only the relevant platform backend is compiled into a platform build.

### Closures

A closure is an anonymous function that may capture surrounding variables:

```rust
thread::Builder::new().spawn(move || {
    run_analysis(ring, stop, shared, phrase)
});
```

`move` transfers captured values into the closure, which is required when the new thread may outlive the spawning stack frame.

### `Arc<T>`

`Arc` means atomically reference-counted ownership. It lets several threads own the same allocation safely. When the final clone is dropped, the allocation is freed.

`Arc` alone does not make mutation safe. The inner type must provide synchronization:

- `Arc<RwLock<AnalysisSnapshot>>` — many reads or one writer;
- `Arc<Mutex<CaptureStatus>>` — one accessor at a time;
- `Arc<AtomicBool>` — lock-free stop flag;
- `Arc<PcmRingBuffer>` — queue methods are thread-safe internally.

### `Mutex<T>`, `RwLock<T>`, and poison recovery

- A `Mutex` permits one holder at a time.
- An `RwLock` permits several readers or one writer.
- A poisoned standard lock means another thread panicked while holding it.

The project usually recovers the inner value with `poisoned.into_inner()` because it prefers cleanup and diagnostics over a second cascading panic. This is a deliberate availability tradeoff; it does not prove the protected state is logically perfect after a panic.

### Atomics and memory ordering

Atomics provide indivisible operations without a mutex. The project uses:

- `AtomicBool` for `stop_requested` and `allow_close`;
- `AtomicU8` for output mode;
- `AtomicU64` for the dropped-sample count.

`Acquire`/`Release` ordering creates synchronization between threads. `Relaxed` is enough for the informational drop counter because no other memory depends on that counter’s ordering. You do not need to be a memory-model expert to explain the intent: flags need cross-thread visibility; metrics only need atomic arithmetic.

### Channels

A channel sends owned messages between threads. During startup, the renderer sends either `RendererReady` or an error through a one-slot synchronous channel. The manager waits up to 12 seconds. A one-shot readiness message is a better fit than repeatedly polling partially initialized GPU state.

### RAII and `Drop`

RAII means resource acquisition is initialization: a resource is tied to an owning value, and cleanup runs in `Drop`.

Strong examples:

- `WasapiPacket` releases its capture buffer exactly once, even on early return;
- `CoreAudioSession` destroys the IO proc, aggregate device, tap, callback context, and description in reverse order;
- `PerformancePowerGuard` restores Windows power state when rendering ends;
- `PerformanceManager::drop` cleans up a remaining performance session.

This is a major interview theme. Native audio code has many early-return paths, so manual cleanup at every return is fragile. RAII centralizes the invariant.

### Panic containment

Long-lived worker bodies are wrapped in `catch_unwind`. If a worker panics, the app logs it and requests peer shutdown. This is not a substitute for correct code, and not every panic is always safely recoverable, but it prevents an unobserved worker death from leaving the UI claiming success.

### Async vs threads in this project

Tauri commands may be `async`, and `wgpu` adapter/device requests return futures. The application uses:

- `tauri::async_runtime::spawn_blocking` for a diagnostic workflow that performs blocking native work;
- `pollster::block_on` to run `wgpu` async setup inside a dedicated renderer thread;
- ordinary OS threads for sustained capture, analysis, and drawing.

“Async” is cooperative task scheduling; a thread is an OS execution resource. They are related but not interchangeable.

### Why `unsafe` is present

The Windows and macOS backends must cross native ABI boundaries. `unsafe` is used around:

- COM initialization and methods;
- raw WASAPI packet pointers;
- native format structures;
- Core Audio callbacks and property calls;
- Objective-C and Core Foundation ownership;
- native keyboard-state functions.

The safe wrappers validate lengths, formats, IDs, and cleanup state around those calls. In an interview, never claim that Rust eliminates all memory risk here. Say that Rust shrinks and documents the manually verified boundary.

---

## 5. TypeScript, React, Tauri, WebGL, `wgpu`, and WGSL basics

### TypeScript

TypeScript adds static types to JavaScript. It is compiled to JavaScript for the webview. Interfaces in `app/src/visuals/types.ts` mirror Rust serialization shapes.

TypeScript types do not validate arbitrary runtime JSON by themselves. In this app, the Rust side is the trusted producer and Tauri/Serde defines the contract. If this became a network-facing API, runtime schema validation would be worth adding.

### React

React renders UI as functions of state.

Important hooks in `ControlApp`:

- `useState` stores settings, runtime status, sources, reports, and errors;
- the first `useEffect` loads displays, sources, native state, and any previous crash report;
- the second `useEffect` polls runtime state every 600 ms and sources every 1.5 seconds while stopped;
- `useMemo` finds the selected display efficiently and declaratively.

The cleanup functions clear timers or prevent state updates after unmount. React does not drive the native renderer. It observes and commands it.

### The transport pattern

`ControlTransport` defines one frontend interface with two implementations:

- `TauriControlTransport` dynamically imports `@tauri-apps/api/core` and invokes Rust commands;
- `BrowserControlTransport` supplies display/settings preview behavior and rejects native-only operations honestly.

This is a form of the adapter/strategy pattern. The controller component does not need `if (native)` around every operation.

### Tauri commands

`#[tauri::command]` exposes a Rust function by name. `tauri::generate_handler![...]` registers the allowed set. Examples:

| Frontend call | Rust command | Meaning |
| --- | --- | --- |
| `getDisplays()` | `get_displays` | Enumerate monitors |
| `getAudioSources()` | `get_audio_sources` | Enumerate process/device routes |
| `getState()` | `get_runtime_state` | Read one serializable snapshot |
| `updateSettings()` | `update_visual_settings` | Sanitize, publish, persist settings |
| `start()` | `start_visuals` | Save settings and transactionally start |
| `stop()` | `stop_visuals` | Idempotently clean the live session |
| `setOutputMode()` | `set_output_mode` | Switch Reactive/Ambient/Black atomically |
| `runDiagnostic()` | `run_connection_diagnostic` | Run a bounded independent probe |

The Tauri content-security policy in `tauri.conf.json` restricts sources and permits the IPC connection needed by Tauri.

### WebGL preview vs native production renderer

This distinction is interview-critical.

The controller preview uses WebGL 2 and GLSL inside the webview. It feeds constant quiet values and zero pulses. The route `/?performance=1` is also a text-free ambient preview. It is useful for visual appearance development.

The production display is a raw Tauri `Window` whose native handle becomes a `wgpu` surface. It does not contain React, a DOM, or a webview. Rust supplies live audio-derived uniforms to WGSL.

The two shaders intentionally share visual concepts and numeric family IDs, but they are separate implementations and can drift if developers update only one.

### `wgpu` and WGSL

`wgpu` is a Rust graphics abstraction that maps to native backends. This crate enables DirectX 12 on Windows and Metal on macOS. WGSL is the shader language used by WebGPU/`wgpu`.

The renderer:

1. creates an instance;
2. creates a surface on the platform main thread;
3. requests a compatible adapter;
4. requests a device and queue;
5. configures the surface;
6. validates the embedded WGSL module;
7. builds a uniform buffer, bind group, and render pipeline;
8. draws a full-screen triangle each frame;
9. presents the surface texture.

---

## 6. A complete live session, step by step

### Application launch

1. The binary entry in `src/main.rs` calls `pulsebridge_lib::run()`.
2. Tauri setup initializes diagnostic logging.
3. It reads `visual-settings.json` from the platform app-config directory.
4. Parsed settings are sanitized; missing or invalid files fall back to defaults.
5. Tauri stores a `PerformanceManager` and `DiagnosticCoordinator` as managed state.
6. The command handler is registered.
7. The controller webview loads the React bundle.
8. React concurrently requests displays, sources, runtime state, and a previous-run report.

### User changes a setting

1. React merges the changed field into local `VisualSettings`.
2. It updates the visible state optimistically.
3. It invokes `update_visual_settings`.
4. Rust sanitizes values again; the UI is not trusted to enforce bounds.
5. The manager replaces its `RwLock`-protected settings.
6. If a session exists, the topmost setting is applied immediately.
7. The settings file is written to a temporary file, flushed, and renamed into place.

The current UI fires updates without awaiting a serialized settings queue. A production refinement would debounce slider changes and serialize persistence to avoid unnecessary disk syncs or competing writes.

### User presses Start

1. React calls `start_visuals` with the current settings.
2. The command refuses to start if a connection diagnostic is active.
3. Rust sanitizes and saves settings.
4. `PerformanceManager` locks its operation mutex, preventing overlapping Start/Stop operations.
5. It stops any previous session, resets errors/status/snapshots, sets lifecycle to `Starting`, and sets output to `Reactive`.
6. It enumerates displays and selects the configured display, falling back to the first if the saved index is gone.
7. It creates a decoration-free, hidden, raw performance window.
8. It positions, sizes, and marks the window full-screen while still hidden.
9. It creates the `wgpu` surface on the platform UI thread. This is especially important on macOS, where obtaining the `NSView` from a worker can panic.
10. It creates the stop flags and a bounded PCM ring using the configured 5–30 second capacity.
11. It starts the analysis worker.
12. It starts the platform audio worker.
13. It starts the renderer worker, which acquires the Windows power guard where applicable.
14. The manager waits up to 12 seconds for the renderer’s first successful present.
15. Only after that present does it hide the cursor, show and focus the performance window, store the session, and set lifecycle to `Running`.

This is **transactional startup**: either all required pieces reach first-frame readiness, or every started piece is rolled back. The audience never sees a half-initialized white/empty window merely because window creation succeeded.

### Steady-state operation

- Audio capture writes mono samples to the bounded ring.
- Analysis drains samples and replaces latest feature/phrase snapshots.
- Rendering reads current settings and latest snapshots at roughly 60 Hz.
- React polls status every 600 ms, but polling is not part of the render loop.
- Output mode is an atomic value, so Reactive/Ambient/Black changes do not restart workers.

### Output modes

- **Reactive**: use live/fading audio features.
- **Ambient**: replace the feature frame with quiet defaults while keeping motion/rendering alive.
- **Black**: continue the pipeline but set a uniform that makes final shader output black.

Black mode is not the same as Stop. Keeping the session alive enables immediate recovery to visuals.

### Stop and cleanup

Stop can originate from the controller, focused Escape, native window close, emergency chord, renderer failure, or worker exit.

1. Set the shared stop flag. The request is idempotent.
2. Drain worker handles and supervise each join with a two-second timeout.
3. Restore cursor visibility, topmost state, and non-fullscreen state.
4. Allow the close handler to close instead of converting close into another stop request.
5. Close only the performance window.
6. Show/focus the controller.
7. Reset capture/renderer status and output mode.
8. Mark the session cleanly exited.

The controller remains the recovery surface. A performance failure should not take away the controls needed to understand or stop it.

---

## 7. Concurrency and data ownership

### Why separate workers

Audio callbacks/capture, FFT analysis, UI updates, and GPU presentation have different timing and failure modes. Combining them on one thread would let a slow task block unrelated work.

| Shared value | Writer | Reader(s) | Synchronization | Policy |
| --- | --- | --- | --- | --- |
| PCM samples | Audio worker | Analysis worker | `ArrayQueue<f32>` | Bounded; overwrite oldest |
| `AnalysisSnapshot` | Analysis worker | Renderer, controller snapshot | `Arc<RwLock<_>>` | Replace latest |
| `PlaybackContext` | Analysis worker/provider | Renderer, controller snapshot | `Arc<RwLock<_>>` | Replace latest |
| `VisualSettings` | Tauri commands | Renderer, manager | `Arc<RwLock<_>>` | Replace sanitized settings |
| `CaptureStatus` | Audio worker | Controller/diagnostics | `Arc<Mutex<_>>` | Current facts/metrics |
| `RendererStatus` | Renderer | Controller/manager | `Arc<Mutex<_>>` | Current renderer facts |
| stop request | Any stop source | All workers | `Arc<AtomicBool>` | Idempotent flag |
| output mode | Controller command | Renderer | `Arc<AtomicU8>` | Immediate latest mode |
| renderer readiness | Renderer | Start transaction | sync channel | One result, 12 s bound |

### Backpressure policy

Backpressure asks what a producer should do when a consumer cannot keep up.

PulseBridge chooses freshness:

- the PCM queue has a fixed capacity;
- overflow drops oldest PCM;
- if analysis sees more than 24,000 queued samples (about 0.5 s), it discards until near the current tail;
- the renderer reads only one current snapshot;
- no missed impact/state events are replayed.

This would be wrong for audio recording, where every sample matters. It is right for live visuals, where displaying a five-second-old beat is visibly wrong.

### Avoiding lock-related stalls

The design holds locks for small clone/replace operations. It does not keep session locks held while joining workers or initializing a GPU. The `operation` mutex serializes lifecycle operations, while the `session` mutex only protects ownership of the current session.

### What “lock-free” does and does not mean

The PCM queue is lock-free. The whole application is not. Status/settings/snapshot objects use locks. An interview answer should say the design uses lock-free transfer only where the high-frequency producer/consumer path benefits, while using simpler locks for low-contention state.

### Data race vs logical race

Rust synchronization prevents undefined-behavior data races. Logical races can still exist—for example, two valid settings saves could arrive in an unintended order. Concurrency correctness still requires lifecycle rules, operation serialization, idempotence, and testing.

---

## 8. Audio capture

### One internal format

Platform APIs may provide different channel counts, sample rates, or sample representations. Both native backends must produce the same downstream contract: mono `f32` at 48 kHz.

This is an example of an **anti-corruption layer** or normalized boundary: analysis code does not care whether samples originated as Windows 24-bit stereo at 44.1 kHz or a macOS float stream.

### Downmixing

Interleaved audio stores channels in sequence per frame. A stereo layout looks like:

```text
L0, R0, L1, R1, L2, R2, ...
```

The converter decodes every channel in a frame, averages the channels, and clamps the result. It validates:

- 1–32 channels;
- sample rate 8 kHz–384 kHz;
- block alignment large enough for the declared sample representation;
- enough packet bytes for the declared frame count.

Supported Windows sample representations are float32 and 16/24/32-bit integer PCM.

### Stateful streaming resampling

Resampling changes sample rate. `StreamingResampler` uses linear interpolation and preserves:

- total input position;
- next fractional output position;
- the previous packet’s last sample.

Keeping state across packets prevents each packet boundary from restarting phase and producing discontinuities. Linear interpolation is simple and low-latency, but it is not a high-end band-limited resampler. That is a reasonable tradeoff for feature extraction; an improvement could use a quality streaming resampler if aliasing measurably affects analysis.

### Windows WASAPI path

WASAPI is the Windows Audio Session API.

#### Source enumeration

The backend:

1. initializes COM in multithreaded mode;
2. finds every `rekordbox.exe` using a process snapshot;
3. marks root candidates vs child candidates;
4. sorts roots first and then PIDs deterministically;
5. exposes `process:auto` even when Rekordbox is closed because it has an explicit default-output fallback;
6. enumerates active render endpoints and marks the current multimedia default.

#### Process capture

On Windows build 20348 or newer, it activates the virtual process-loopback device for a Rekordbox PID, including the target process tree. It tries deterministic candidates one by one.

If the OS is too old, no process is found, activation fails, or the process route never produces live samples, the automatic source records the preferred failure and tries default render-endpoint loopback. It does not relabel fallback audio as process audio.

If the user selected a particular output device, that route remains selected and does not automatically abandon it merely because it is silent.

#### Packet loop

The capture client asks WASAPI for the actual shared-mode mix format, validates it, starts loopback, and drains all currently available packets. For each packet it:

1. handles the WASAPI silent flag;
2. decodes/downmixes if needed;
3. resamples to 48 kHz;
4. computes packet RMS/peak for status;
5. pushes samples to the ring;
6. releases the native packet exactly once;
7. updates distinct status facts.

The `WasapiPacket` `Drop` implementation is a textbook RAII answer: `GetBuffer` must always pair with `ReleaseBuffer`, even if conversion or a later operation returns an error.

#### Route retry policy

- Process route without any prior signal: retry after 3 seconds of silence.
- Process route after signal existed: treat 5 seconds of loss as a reconnect condition.
- Default fallback: retry the preferred route after 30 seconds of silence.
- Explicit selected output: do not switch routes solely due to silence.

The visual reactivity fade begins sooner than route reconnection. User experience and transport policy are separate layers.

### macOS Core Audio path

On macOS 14.2 or newer, the backend:

1. discovers Rekordbox through `NSWorkspace`;
2. translates its PID to a Core Audio process object;
3. creates a private, unmuted process tap;
4. creates a private aggregate device containing that tap;
5. installs and starts an IO proc callback;
6. converts callback buffers to mono 48 kHz;
7. publishes flow/status outside the callback path;
8. destroys resources in reverse order when the process stops or capture ends.

Tap creation/destruction symbols are resolved at runtime. That lets the application launch on older macOS and report the exact 14.2 requirement rather than fail to load because a symbol is missing.

Audio capture permission is controlled under **System Settings → Privacy & Security → Screen & System Audio Recording**. Permission denial is an operational state, not something the app should bypass.

Unlike Windows automatic capture, the current macOS backend does not claim a default system-output fallback. That asymmetry should be stated honestly.

### Status facts are intentionally separate

| Fact | Meaning |
| --- | --- |
| `rekordboxDetected` | A matching process exists |
| `captureInitialized` | Native client/tap setup succeeded |
| `packetsReceived` | The API delivered packet/callback data |
| `nonSilentSamplesReceived` | Some converted samples exceeded the silence threshold |
| `reactiveReady` | Live non-silent input is currently suitable for response |
| `route` | Process, selected output, or explicit fallback actually used |
| `sampleFlow` | Unavailable, waiting, flowing, or silent |

A found process is not proof of sound. An initialized client is not proof of non-silent samples. Samples are not proof of Rekordbox phrase metadata. Keeping these facts separate prevents misleading green “connected” states.

### Privacy

PCM exists only in memory. The project does not write captured audio, log samples, upload it, or save library/track paths. Diagnostic metrics include aggregate counts, RMS, peak, format, and route facts.

---

## 9. Signal processing and musical analysis

### Analysis timing constants

| Constant | Value | Interpretation |
| --- | ---: | --- |
| Internal sample rate | 48,000 Hz | 48,000 mono samples per second |
| FFT size | 2,048 samples | About 42.67 ms of audio |
| Hop | 960 samples | 20 ms; up to about 50 analysis frames/s |
| FFT bin width | 23.4375 Hz | `48,000 / 2,048` |
| Backlog threshold | 24,000 samples | About 0.5 s before stale PCM is discarded |
| Feature-history cap | 6,000 frames | Roughly 120 s at 50 frames/s |

### RMS loudness

RMS is root mean square:

```text
RMS = sqrt((x1^2 + x2^2 + ... + xN^2) / N)
```

It estimates signal magnitude while treating negative and positive waveform values symmetrically.

### Hann window

The analyzer multiplies 2,048 samples by a Hann window before the FFT. A finite slice usually starts and ends at unrelated waveform phases; treating it as periodic creates spectral leakage. The Hann window tapers the ends toward zero, reducing that discontinuity.

### FFT

The fast Fourier transform converts a time-domain window into complex frequency bins. Magnitude is derived from each complex bin. PulseBridge ignores DC bin zero and examines bins through Nyquist.

The four bands are approximately:

- sub: 20–80 Hz in product language; implementation assigns bins below 80 Hz after skipping DC;
- bass: 80–250 Hz;
- mids: 250–2,500 Hz;
- highs: 2,500–12,000 Hz.

Energy is accumulated as squared magnitude, averaged by bin count, then square-rooted. Frequencies above 12 kHz are not used by the band features.

### Spectral flux and onset

Spectral flux sums only positive changes in magnitude from the previous spectrum. It answers: “How much new spectral energy appeared?” A kick, snare, or abrupt musical event often creates high positive flux.

Flux is rolling-normalized and raised to power 1.3 to form onset strength, emphasizing stronger changes.

### Rolling normalization

Absolute thresholds fail because songs have different mastering levels. Each channel keeps a bounded recent history and periodically sorts it to estimate:

- 10th percentile (`low`);
- 50th percentile (`median`);
- 92nd percentile (`high`).

It forms a floor from 72% low + 28% median, then maps the current value between that floor and high into `0..1`.

This makes a quiet track’s relative peak capable of driving strong visuals. The tradeoff is adaptation time and CPU cost: six rolling histories can sort up to 6,000 values every ten analysis frames. If profiling showed this to be expensive, streaming quantile estimators or histograms would be a likely optimization.

### Fast and slow energy envelopes

The normalized energy feeds two exponential smoothers:

- fast coefficient: 0.32 per analysis update;
- slow coefficient: 0.035.

Their difference indicates trend. Fast above slow suggests rising energy; fast below slow suggests falling energy.

### Rhythm tracker

The rhythm tracker is intentionally lightweight, not a full beat-grid engine.

1. Treat onset above 0.68 as a possible beat if at least 0.24 s passed.
2. Accept beat intervals from 0.24 to 1.1 s, roughly 250 to 54.5 BPM.
3. Keep the last 12 intervals.
4. Use the median as the estimated interval, which resists outliers better than a mean.
5. Compute repeating phase as elapsed/interval modulo 1.
6. Compute a quickly decaying pulse after the last accepted onset.

It provides visually useful phase/pulse, but it does not claim DJ-grade beat-grid accuracy.

### Musical state

`MusicalStateTracker` emits one of:

- Quiet;
- Flow;
- Groove;
- Build;
- Impact;
- Peak;
- Breakdown.

It considers raw silence, normalized energy, fast/slow trends, highs, onset density, bass jumps, beat strength, prior state, and bounded history.

#### Impact

Impact has a weighted score using onset, flux, positive energy jump, positive bass jump, beat strength, and build context. A score over 0.72 can trigger Impact if the 1.25-second cooldown has elapsed. Impact is held/released for about 0.42 seconds.

#### Hysteresis and dwell

For ordinary states, the tracker first records a candidate and the time it became a candidate. It changes the current state only if the candidate remains long enough. Dwell ranges from 0.55 to 0.9 seconds depending on state.

This is hysteresis: entry does not happen on one noisy frame. It prevents rapid state flicker and therefore rapid visual identity changes.

### From features to `VisualInputFrame`

The analyzer produces a detailed `FeatureFrame`. The worker condenses it for visuals:

| Visual field | Source/use |
| --- | --- |
| `energy` | fast normalized energy; speed/brightness/density |
| `sub` | sub-band energy; low-frequency accent |
| `bass` | bass deformation/depth |
| `mids` | form contribution |
| `highs` | detail/sparkle |
| `beatPhase` | continuous rhythmic travel |
| `beatPulse` | short camera/shape impulse |
| `onset` | brief activity/accent |
| `impact` | transition/modifier/optional flash trigger |
| `state` | macro musical category |
| `reactivity` | freshness/user-scaled live response |

### Audio freshness and graceful loss

`AnalysisSnapshot::visual_input(now)` separates stored feature values from freshness:

- age 0–300 ms: full reaction;
- age 300 ms–2 s: linear fade toward ambient defaults;
- age 2 s or more: zero reaction, Quiet state, ambient feature values.

The renderer continues using monotonic time, so geometry still moves. It does not freeze the last bright impact on screen.

---

## 10. Phrase inference and the Rekordbox boundary

### Playback context as an abstraction

`PlaybackContext` is designed so a future source could provide:

- stable track ID;
- session-relative/live position;
- playing state;
- active deck;
- phrase segment and progress;
- provenance and update time.

The current `AudioInferredPhraseProvider` leaves track/deck identity absent. This allows the director interface to remain stable if a documented provider is added later.

### Current audio-inferred provider

The provider samples compact observations every 250 ms. It stores at most 256 observations—about 64 seconds—and stores energy/onset values, not PCM.

Default phrase rules:

- minimum phrase dwell: 8 seconds;
- maximum phrase dwell: 28 seconds;
- suggested phrase must normally remain stable for 2 seconds;
- a strong impact plus novelty may mark a boundary after minimum dwell;
- phrase index increases monotonically.

State maps approximately to:

| Musical state | Inferred phrase |
| --- | --- |
| Quiet at session start | Intro |
| Later Quiet | Down |
| Flow/Groove | Verse |
| Build | Up |
| Impact/Peak | Chorus |
| Breakdown | usually Down; periodic Bridge |

Novelty compares average energy in older vs newer halves of history and adds recent onset activity. Confidence is intentionally bounded below certainty.

### Freshness rules

- Status labels phrase context stale after 2 seconds.
- The scene director accepts phrase direction for up to 5 seconds.
- `PhraseRouter` holds a future external provider for at most 5 seconds before using inference.

These thresholds serve different UI and continuity purposes. The production provider updates continuously, so they mainly protect a future external source or failure.

### Honest interview answer

> We investigated documented Rekordbox interfaces but did not find a supportable live outward contract containing phrase boundaries, track/deck identity, and playhead/beat-grid state. Instead of scraping private databases or claiming inferred structure as metadata, the current app publishes an `AudioInferred` context with explicit provenance. The interface is provider-oriented so a documented, read-only, version-gated source could be added later without coupling it to audio capture or rendering.

---

## 11. Scene direction

### Why have a director after analysis

FFT features change every 20 ms. If raw values chose whole scenes, visual identity would flicker. The scene director separates:

- **macro direction**: phrase/state chooses a stable base family and transition timing;
- **micro animation**: current energy/bands/onsets animate that family every frame.

### Base visual families and stable IDs

| ID | Rust name | Visual idea |
| ---: | --- | --- |
| 0 | Wave Field | Layered moving waves |
| 1 | Bloom | Radial flower/drop expansion |
| 2 | Pulse Geometry | Rings and beat geometry |
| 3 | Warp Tunnel | Radial depth/tunnel motion |
| 4 | Fluid Ribbons | Flowing ribbons |
| 5 | Prism Beams | Directed light beams |
| 6 | Star Trails | Moving points and trails |
| 7 | Kaleidoscope | Folded radial symmetry |

These IDs must match Rust, WGSL, and the GLSL preview.

### Manual style mapping

| UI style | Base family |
| --- | --- |
| Fluid | Fluid Ribbons |
| Waves | Wave Field |
| Pulse | Pulse Geometry |
| Tunnel | Warp Tunnel |
| Burst | Bloom |

Auto uses phrase/state direction. Manual mode still receives live feature animation, but it fixes the family. The current director clears optional modifiers in manual and Chill modes, reducing visual surprise.

### Phrase candidates

Each phrase kind has a small allowed family set. For example, Up favors Warp Tunnel, Prism Beams, or Fluid Ribbons; Chorus favors Pulse Geometry, Kaleidoscope, or Bloom.

Selection combines a session seed and direction key. Given the same seed/key/history it is deterministic, while a new session seed permits variation. The recent history is capped at eight and prevents selecting the same primary for a third consecutive phrase when alternatives exist.

### Dwell and crossfade

A base normally remains for at least 12 seconds. A transition renders the outgoing and incoming bases together with weights whose sum is normalized to one. Smoothstep easing avoids a linear-looking hard handoff.

Transition durations depend on phrase and intensity. Fast, energetic changes such as Chorus/Fill can be around 0.8 seconds at Balanced; quiet Intro/Down/Outro transitions can be around 2.4 seconds. Chill lengthens and Wild shortens these durations.

Exactly one base is active outside a transition. “At most two bases” means the two are the outgoing and incoming versions of one normalized crossfade, not independent competing scenes.

### Modifiers and IDs

| ID | Modifier | Effect |
| ---: | --- | --- |
| 0 | Palette Drift | Moves cyclic palette coordinates |
| 1 | Beat Zoom | Increases beat-driven camera/UV zoom |
| 2 | Bass Warp | Bass-driven coordinate deformation |
| 3 | High Sparkle | High-frequency sparkle points |
| 4 | Echo Trails | Adds a low-brightness trail band |
| 5 | Mirror Fold | Mirrors/folds coordinates |
| 6 | Chromatic Split | Adds colored edge energy |
| 7 | Impact Bloom | Adds a brief radial impact front |

Modifiers rotate on a roughly 16-second key. Normal modifiers use attack/hold/release envelopes; Impact Bloom attacks in 40 ms, holds about 160 ms, and releases over 500 ms. At most two are active. Compatibility rules prevent redundant or visually problematic combinations.

As modifier load rises, `ScenePlan::normalized` lowers the maximum brightness budget rather than letting effects multiply luminance without bound.

### Scene budgets

Every plan supplies motion, detail, density, and brightness. Phrase type sets a base budget; Chill/Balanced/Wild scales it; current energy further adjusts motion. Values and mixes are finite-clamped so NaN/infinite or out-of-range inputs cannot reach the shader unchecked.

### Palettes

Each palette has four colors. The shader samples four equal cyclic transitions:

```text
A -> B -> C -> D -> A
```

This makes palette coordinates continuous at wrap. Auto direction currently resolves through scene/phrase direction; the palette module also has a musical-state mapping. Fixed choices override direction. CPU palette values interpolate with a time constant around 0.85 seconds to avoid abrupt color cuts.

---

## 12. Native GPU rendering

### Why a native renderer

A native window removes the controller webview from the performance path and gives explicit surface/device error handling. It also makes first-present readiness meaningful: success means a real native surface accepted a real GPU frame.

### Main-thread surface creation

The raw window is created by Tauri. `prepare_renderer_surface` creates a `wgpu::Instance`, then schedules surface creation with `window.run_on_main_thread`. The operation has a five-second timeout.

The surface object and dimensions are then moved to the renderer worker. GPU drawing remains off the controller/UI thread.

### Adapter selection

The live renderer attempts:

1. high-performance hardware compatible with the surface;
2. compatible low-power hardware;
3. forced software fallback.

It records adapter name, backend, driver, driver info, device type, surface format, present mode, and whether fallback was used.

### Validation and pipeline

The renderer uses `wgpu` validation error scopes around shader and pipeline creation. It embeds `performance.wgsl` into the executable with `include_str!`, so production does not need to locate a loose shader file.

The uniform buffer is `UNIFORM | COPY_DST`. Each frame the CPU writes one `VisualUniforms` value, binds it, draws three vertices, submits, and presents.

### Uniform layout

The Rust struct and WGSL struct have matching groups of four `f32` values:

| Uniform group | Main contents |
| --- | --- |
| `resolution_time` | width, height, elapsed seconds, frame delta |
| `music` | smoothed energy, bass, mids, highs |
| `pulse` | beat phase, smoothed beat pulse, onset, white flash |
| `visual` | effective motion, detail, zoom/depth, brightness |
| `color_a..d` | four palette colors |
| `style_a` | primary ID, secondary ID, normalized weights |
| `style_b` | saturation, reactivity, variation seed, black flag |
| `effects` | sub, impact, color-change amount, spare |
| `scene` | scene motion, detail, density, brightness |
| `modifiers` | two `(ID, strength)` slots; `-1` means absent |

`#[repr(C)]`, `Pod`, and `Zeroable` are important because GPU memory layout must be predictable. Changing fields requires keeping Rust and WGSL layouts aligned.

### Smoothing

Raw visual inputs pass through asymmetric exponential envelopes. Attack constants are shorter than release constants for energy bands, beat, onset, and impact. Beats arrive quickly, then decay more smoothly. Reactivity itself fades more slowly.

The generic form is:

```text
amount = 1 - exp(-delta / time_constant)
new = current + (target - current) * amount
```

Using elapsed time instead of a fixed per-frame amount makes behavior less dependent on frame rate.

### Flash safety

White flash defaults to Off. Moderate and High have different caps, cooldowns, and decay constants. A flash triggers only on a threshold crossing and cooldown eligibility. The colored/spatial impact continues even when white flash is disabled.

This is both a product decision and a safety engineering decision.

### Shader structure

The vertex shader emits a full-screen triangle using only `vertex_index`. The fragment shader:

1. converts pixel position to aspect-correct coordinates;
2. applies global modifiers such as Beat Zoom, Bass Warp, and Mirror Fold;
3. evaluates the primary base and optional crossfade base;
4. applies vignette, brightness, onset/sub accents;
5. adds compatible sparkle/trail/chromatic/impact modifiers;
6. optionally mixes toward white for the bounded flash;
7. caps luminance;
8. applies saturation, tone mapping, and gamma-like shaping;
9. outputs black if Black mode is active.

The shader uses cyclic palette coordinates and whole-number angular frequencies to avoid an `atan2` seam at negative X.

### Frame pacing and errors

The target interval is 16,666,667 ns, about 60 Hz. The renderer uses monotonic `Instant`, clamps frame delta to `0.001..0.1`, and sleeps until the next target. If it falls behind, it resets the next target rather than trying to burst-render missed frames.

Surface outcomes:

- Success: render normally.
- Suboptimal: reconfigure and use the frame.
- Lost: recreate the surface on the main thread.
- Outdated: reconfigure and skip this frame.
- Timeout/Occluded: skip this frame.
- Validation: fail the session.

Uncaptured GPU errors and device loss set renderer status to Failed and request orderly session stop.

---

## 13. Diagnostics and reliability

### Why diagnostics are a separate workflow

“Start full-screen and see whether the app disappears” is a poor troubleshooting method. The connection diagnostic runs independently of a live show and returns bounded, structured evidence.

The app prohibits a diagnostic while live visuals run and prohibits live Start while a diagnostic is active. `DiagnosticCoordinator` serializes probes and exposes one cancellation flag.

### Diagnostic modes

| Mode | What it tests |
| --- | --- |
| Audio only | Source discovery, capture initialization, first packet, non-silent signal, reactive readiness |
| Renderer only | Adapter, device, WGSL validation, and pipeline creation without a full-screen surface |
| Safe renderer | Conservative low-power/downlevel device request with fallback attempt |
| Full startup | Audio probe + renderer probe + temporary hidden raw surface + first present + cleanup |

Full startup does not enter the normal full-screen session. It creates a hidden 640×360 diagnostic window, presents a real frame, stops the worker with a bound, and closes it.

### Time bounds

Key diagnostic bounds include:

- capture initialization: 4 seconds;
- first packet: 4 seconds;
- non-silent samples: 5 seconds;
- hidden first frame: 8 seconds;
- worker join: 2 seconds.

Cancellation is checked during waits, not only between whole phases.

### Structured report

A versioned report contains:

- UUID report and session IDs;
- mode, start time, duration, verdict;
- failure stage and stable code;
- ordered stage results and details;
- audio facts/format/metrics;
- renderer adapter/backend/driver/surface facts;
- bounded recent log events;
- log and report paths.

Stable codes are more useful than matching prose. Examples include `REKORDBOX_PROCESS_NOT_FOUND`, `WINDOWS_BUILD_UNSUPPORTED`, `GPU_SHADER_VALIDATION_FAILED`, `GPU_FIRST_FRAME_TIMEOUT`, and `UNEXPECTED_PROCESS_EXIT`.

### Logs and durable crash-stage reconstruction

`pulsebridge.log` is newline-delimited JSON. It rotates at about 1 MB through `.1`, `.2`, and `.3`. A bounded in-memory recent list holds 128 records.

`last-session.json` is synchronously replaced around risky stages. It records whether the last session was in progress or cleanly exited and the last stage/code/message. If the next launch sees an unfinished marker, it reconstructs `previous-run-report.json` with `UNEXPECTED_PROCESS_EXIT`.

This does not identify every native crash root cause, but it narrows the last durable stage even if the process vanished before an ordinary error could return.

Diagnostic report files are written to unique temporary files, flushed, and replaced. Windows uses `MoveFileExW` with replace-existing/write-through flags; other platforms use rename.

### Failure containment

The design uses several layers:

- bounded buffers and histories;
- latest-value rendering;
- ambient fallback for stale audio;
- deterministic route fallback reporting;
- startup rollback;
- first-frame readiness;
- worker panic containment;
- stale-session reaping during controller status polling;
- native GPU error callbacks;
- idempotent stop flag;
- bounded worker joins;
- RAII cleanup;
- controller survival.

### Lifecycle state machines

Overall lifecycle:

```text
Stopped -> Starting -> Running
                      <-> Recovering
Any active state -> Failed
Any state --Stop--> Stopped
```

Renderer lifecycle and audio capture lifecycle are separate. For example, the renderer can be Running while audio is Recovering, producing ambient output. A single boolean would lose that information.

### Escape and emergency exits

The renderer checks:

- focused Escape;
- Windows `Ctrl+Alt+Shift+F12`;
- macOS `Control+Option+Shift+F12`.

Normal OS close is intercepted and converted to the same stop request until cleanup allows the actual close. Multiple stop sources converge on one idempotent mechanism.

### Sleep prevention

On Windows, `PerformancePowerGuard` requests system/display availability only for the renderer worker’s lifetime. Its `Drop` restores normal power behavior. This avoids leaving a permanent machine-wide setting after a failure.

---

## 14. Frontend controller behavior

### Initialization and polling

The controller loads four facts concurrently: displays, audio sources, runtime state, and a previous-run report. Concurrent loading lowers startup wait and keeps concerns independent.

Runtime polling every 600 ms is appropriate for human-visible status. It is intentionally much slower than audio or rendering. Audio-source enumeration pauses while a session is running, avoiding unnecessary native enumeration during performance.

### Optimistic settings

The UI updates its local state immediately, then sends the native write without blocking interaction. Rust sanitization remains authoritative. If persistence fails, the controller shows the error.

A possible improvement is to debounce rapid slider writes and reconcile with the last confirmed native snapshot.

### Safety and status UI

The controller exposes:

- display and source selection before start;
- live route, capture, flow, reactivity, phrase provenance, and renderer facts;
- opt-in flash controls;
- Reactive/Ambient/Black while live;
- immediate Stop;
- copyable human and JSON reports;
- the logs folder.

This is not just presentation. The UI mirrors the architecture’s insistence that detection, initialization, signal, provenance, and rendering are different facts.

### Browser behavior

Without `window.__TAURI_INTERNALS__`, the browser transport:

- reports the current screen as a preview display;
- persists preview settings to `localStorage`;
- reports native capture as unavailable;
- rejects Start/diagnostics with clear messages.

It never fabricates audio reaction to make the demo look connected.

---

## 15. Settings and persistence

### Defaults and valid ranges

| Setting | Default | Sanitized range/values |
| --- | --- | --- |
| display ID | 0 | valid index chosen at start or first fallback |
| source | `process:auto` | empty becomes `process:auto` |
| PCM capacity | 10 s | 5–30 s |
| style | Auto | Auto/Fluid/Waves/Pulse/Tunnel/Burst |
| intensity | Balanced | Chill/Balanced/Wild |
| palette | Auto | ten named choices |
| flash | Off | Off/Moderate/High |
| topmost | false | boolean |
| reactivity | 1.0 | 0–1.5 |
| motion | 1.0 | 0.25–1.75 |
| brightness | 1.0 | 0.25–1.25 |
| color change | 1.0 | 0–1.5 |
| flash strength | 1.0 | 0–1.0 |

Sanitization occurs both when loading and when commands receive settings. This is defense in depth against corrupted files or unexpected callers.

### Native vs browser storage

- Native app: platform config directory, `visual-settings.json`.
- Browser preview: `localStorage` key `pulsebridge-visual-settings`.

Raw audio is never in either settings store.

---

## 16. Build, test, CI, and release

### Prerequisites

- Node.js 22.12 or newer;
- Rust 1.87 or newer according to the package metadata;
- platform build tools required by Tauri/native dependencies.

### Main commands

```bash
npm ci
npm run dev
npm run tauri -- dev
```

`npm run dev` is browser-only. `npm run tauri -- dev` is the desktop path required for live capture and native rendering.

### Verification commands

```bash
npm run lint
npm run typecheck
npm run build
cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml
```

At the time this guide was created, lint, typecheck, frontend production build, and all 44 Rust unit tests passed on the current macOS development workspace.

### What tests cover

The Rust suite includes:

- audio format conversion/downmix/resampling continuity;
- bounded overwrite behavior;
- route retry policy;
- exactly-once packet release gating;
- audio freshness fade;
- normalization and musical-state dwell/cooldown/history;
- phrase dwell, bounded history, and stale-provider fallback;
- deterministic normalized scene plans;
- crossfades, family repetition, modifier compatibility/envelopes/budgets;
- palette continuity and interpolation;
- flash safety;
- lifecycle transitions, startup rollback, close routing, stale-session cleanup;
- diagnostic schemas, stable codes, cancellation, rotation, crash reconstruction;
- WGSL parsing/validation;
- renderer failure requesting orderly stop.

### What tests do not prove

Unit and CI checks do not prove:

- actual Rekordbox capture on a target Windows/macOS machine;
- permission behavior on every macOS version;
- behavior with ASIO and `PC MASTER OUT` configurations;
- latency under a full party workload;
- GPU/driver/display compatibility across real hardware;
- long-duration memory stability;
- Windows installer signing or macOS signing/notarization.

The CI matrix performs Linux frontend checks plus Windows/macOS native compile, lint, test, and unsigned bundle builds. Real hardware validation is still required.

### Packaging

- Windows: unsigned NSIS `.exe` installer.
- macOS: unsigned `.app` and `.dmg`.

The Windows script installs Node, Rust, C++ Build Tools/SDK when missing, runs checks, builds NSIS, copies the installer to `transfer-ready`, and writes a SHA-256 file. End users do not need build tools after installing the finished package.

Release profile retains debug information and does not strip symbols, helping post-crash diagnosis. Production signing requires real credentials and is not simulated.

---

## 17. Design patterns and engineering principles

### Adapter/strategy pattern

- Browser vs Tauri `ControlTransport` implementations.
- Windows/macOS/stub `PlatformCaptureBackend` implementations.
- `PhraseProvider` abstraction for inferred vs possible future external context.

### State machine

Explicit lifecycle enums and transition functions are easier to test than loosely related booleans.

### Producer-consumer

Audio produces PCM; analysis consumes it. The bounded queue decouples their scheduling.

### Latest-value state

Analysis and phrase workers replace snapshots. The renderer consumes the newest state instead of replaying events.

### RAII

Native buffers, taps, devices, power state, and sessions are tied to owning values and `Drop`.

### Transaction

Startup accumulates resources but publishes a live session only after first frame. Any earlier failure rolls back accumulated resources.

### Circuit breaker/degraded mode idea

Audio loss does not repeatedly throw errors onto the audience screen. The visual subsystem degrades to ambient while capture retries. Black and Ambient are manual safety modes.

### Provenance

Phrase data carries an explicit source. Fallback audio carries an explicit route. Diagnostics carry stable stages/codes. The system tries to preserve not only a value but how that value was obtained.

### Defense in depth

Settings are typed in TypeScript, deserialized in Rust, sanitized on load/update/start, clamped again at scene/uniform boundaries, and limited by shader luminance/flash rules.

---

## 18. Important tradeoffs and honest limitations

### Heuristic beat and phrase inference

The rhythm tracker is onset-interval based, and phrase inference is state/novelty/dwell based. They are lightweight, bounded, and streaming, but not equivalent to offline music-information-retrieval models or Rekordbox’s analyzed grid.

Possible improvements:

- multi-hypothesis tempo tracking and half/double-tempo handling;
- phase-locked beat tracking;
- genre-aware or learned segmentation;
- confidence calibration against labeled music;
- a documented provider for actual deck/track/phrase data.

### Linear resampler

Linear interpolation is simple and phase-continuous across packets but has weaker anti-aliasing than a band-limited resampler. Measure analysis impact before adding complexity.

### Rolling percentile cost

Sorting bounded histories is understandable and robust but not asymptotically optimal. Profiling could motivate an approximate quantile data structure.

### Duplicated shaders

WGSL production and GLSL preview deliberately serve different runtimes, but duplicated family/palette logic creates synchronization risk. Tests validate WGSL syntax, not visual equivalence. Shared generated constants or screenshot/golden tests could reduce drift.

### Polling controller state

Polling is simple and resilient. Events/subscriptions could reduce calls and improve immediacy, but must not couple rendering to webview health.

### Settings write frequency

Writing/flushing on every slider event favors immediate persistence but may do unnecessary I/O and can create ordering pressure. Debounce and a single persistence worker would improve it.

### Soft frame pacing

Sleeping toward 60 Hz is portable and simple, but it is not synchronized precisely to every display refresh rate. Surface present mode and OS scheduling still control actual presentation.

### Audio fallback semantics

Windows automatic fallback may capture all sounds on the default output, not only Rekordbox. The route is explicit in status, but users should understand the scope. macOS has no corresponding current system-output fallback.

### Hardware evidence still required

The source and tests are extensive, but target Rekordbox versions, Windows builds, permissions, GPU drivers, display disconnects, sleep/wake, and long performance sessions need real-machine validation.

### Ableton Link is not implemented in the current runtime

`Cargo.toml` declares an empty `link-integration` feature, and documentation describes Link only as a possible optional timing enhancement. No current module consumes Ableton Link data. The active clock is the audio-derived rhythm tracker, so do not present Link as an implemented integration in an interview.

---

## 19. Debugging playbooks

### Start button is disabled

Check:

1. Is this the native Tauri app rather than the browser preview?
2. Was a display found?
3. Is the selected source present and `available`?
4. Is a diagnostic currently running?

### Process detected, but no reactive visuals

Do not stop at “Detected.” Check in order:

1. capture initialized;
2. actual route;
3. packets received;
4. non-silent samples received;
5. reactive ready;
6. audio age;
7. output mode is Reactive;
8. music reactivity is above zero.

On Windows with ASIO, Rekordbox may not send audio through the ordinary Windows output unless `PC MASTER OUT` or an appropriate route is enabled.

### Visual window never appears

Run Renderer only, Safe renderer, then Full startup diagnostics. Inspect stages for:

- surface creation;
- adapter selection;
- device creation;
- WGSL validation;
- pipeline creation;
- first frame present;
- worker stop/join.

The live window remains hidden until first present, so “never appeared” often means the transaction correctly avoided revealing incomplete output.

### App ended unexpectedly

On next launch, inspect `previous-run-report.json`, report/session ID, last stage, timestamp, and `pulsebridge.log`. On Windows correlate that time with Event Viewer or Windows Error Reporting and record faulting module/exception code.

### Visuals freeze after audio stops

Expected behavior is fade to ambient, not freeze. Inspect:

- whether `AnalysisSnapshot::visual_input` is being called with a current monotonic time;
- audio age and reactivity;
- renderer worker status;
- whether output is still presenting frames;
- device-lost or surface errors.

### Colors or families differ between preview and production

Check ID and palette alignment across:

- `src-tauri/src/visuals/director.rs`;
- `src-tauri/shaders/performance.wgsl`;
- `app/src/visuals/model.ts`;
- `app/src/visuals/shader.ts`.

Remember that the browser intentionally receives ambient constants, so timing/reactivity will not match.

---

## 20. Interview questions with model answers

### Interview truth rule

Understanding a codebase and personally authoring every part are different claims. Be exact about your contribution. If you did not write a subsystem, say “the project uses…” or “I traced and can explain…” rather than “I implemented…”. Interviewers generally respect a precise scope of ownership; an authorship claim that collapses under follow-up is much worse than saying you inherited, reviewed, tested, or extended the design.

### Product and architecture

#### 1. What does the project do?

It captures supported live audio associated with a DJ’s Rekordbox setup, converts it into musical features, and drives full-screen abstract GPU visuals on another display. A React/Tauri controller handles settings and diagnostics, while Rust workers own capture, analysis, direction, and rendering.

#### 2. Why Tauri instead of a normal website?

A browser cannot directly use the required native process-loopback/Core Audio tap APIs or provide the same raw native GPU surface and lifecycle control. Tauri still allows a productive React UI while keeping OS integration in Rust.

#### 3. Why is React not part of the render path?

Webview scheduling, garbage collection, layout, or minimization could introduce latency. The controller should be allowed to lag without delaying a frame. Rust workers and `wgpu` keep the performance path independent.

#### 4. Describe the end-to-end data flow.

Platform capture negotiates native audio, downmixes and resamples to mono 48 kHz, and writes a bounded queue. Analysis creates FFT/rhythm/state features and an inferred phrase context. Scene direction converts macro context into one base, an optional crossfade, and bounded modifiers. The renderer smooths current values, writes a uniform buffer, draws a full-screen WGSL triangle, and presents it.

#### 5. Is it really reading Rekordbox phrases?

No. The current source is audio inference with explicit `AudioInferred` provenance. No documented live Rekordbox phrase/playhead/deck API is used. The architecture has a provider boundary for a future documented source.

#### 6. Does Windows capture only Rekordbox?

The preferred modern Windows route is process loopback including the Rekordbox process tree. If automatic process capture cannot work, the app can explicitly use default-output loopback, which may include other system audio. A selected output route also captures that output. The UI/report preserves the actual route.

### Rust and safety

#### 7. Explain ownership in a project example.

Worker threads need values that outlive the Start function, so the manager clones `Arc`s and owned strings into `move` closures. Each worker handle is owned by the session, which later drains and joins them. Resource ownership makes cleanup paths explicit.

#### 8. Why use `Arc<Mutex<T>>`?

`Arc` provides shared lifetime across threads; `Mutex` provides exclusive mutation. For example, the audio worker updates `CaptureStatus` while the controller reads it. `Arc` alone would not make mutation safe.

#### 9. Why use `RwLock` for analysis/settings?

There is one writer and potentially several short-lived readers. `RwLock` expresses that shape and permits concurrent reads. The protected work is kept small: clone/read or replace a snapshot.

#### 10. Why atomics for stop and output mode?

They are tiny, frequently checked values that do not require a compound invariant. An atomic avoids taking a lock each audio/render iteration and provides direct cross-thread visibility.

#### 11. What is RAII and where is it used?

RAII ties resource lifetime to an object. `WasapiPacket::Drop` releases a native packet, `CoreAudioSession::Drop` tears down tap/device/callback resources, and `PerformancePowerGuard::Drop` restores power settings. Cleanup happens on normal scope exit and error returns.

#### 12. Does Rust make the native code completely safe?

No. FFI and raw pointers require `unsafe`, and the programmer must uphold native API invariants. Rust helps contain those regions and makes safe ownership/cleanup around them easier to audit.

#### 13. `Option` vs `Result`?

`Option` represents absence without necessarily being an error, such as no last audio timestamp. `Result` represents an operation that can succeed or fail with a reason, such as creating a GPU surface.

#### 14. Why recover poisoned locks?

A worker panic should be logged and cleaned up without causing every other thread to panic when it touches status. Recovering the inner value favors availability. It is a deliberate tradeoff and does not mean the state is guaranteed semantically valid.

### Concurrency and latency

#### 15. How does the project prevent unbounded lag?

The PCM queue, feature history, phrase observations, scene history, recent logs, startup waits, and join waits are bounded. PCM overflow drops oldest samples, analysis discards excessive backlog, and rendering consumes latest state rather than queued events.

#### 16. Does a ten-second PCM buffer add ten seconds of latency?

No. It is maximum capacity. The analysis worker continually drains it. Latency comes from capture packet scheduling, the analysis window/hop, worker scheduling, smoothing, and the next rendered/presented frame—not from intentionally waiting for the buffer to fill.

#### 17. Why drop old audio instead of blocking capture?

Blocking capture risks native buffer overrun and makes current visuals late. This is a live visualization, not a recorder, so current data is more valuable than complete historical data.

#### 18. Is the system lock-free?

Only the high-frequency PCM queue is lock-free. Snapshot/status/settings use locks, and lifecycle uses a mutex. The design chooses the simplest appropriate primitive for each shared value.

#### 19. What happens if React freezes?

Native audio, analysis, and rendering continue because they do not wait on React. Status polling and settings control may pause, but the show path remains alive and native shortcuts still work.

#### 20. What happens if analysis falls behind?

If the PCM queue exceeds about half a second, analysis removes old samples until it is near the current tail. The renderer keeps reading its latest snapshot and eventually fades stale audio toward ambient.

### Audio/DSP

#### 21. Why normalize all audio to 48 kHz mono?

It gives analysis one predictable contract across native formats. FFT band math and timing constants remain consistent, and visuals do not need stereo localization.

#### 22. Why use a Hann window?

It tapers FFT-frame edges and reduces spectral leakage caused by treating an arbitrary finite slice as periodic.

#### 23. What is spectral flux?

It sums positive increases in spectrum magnitudes between adjacent frames. It is useful for detecting newly arriving energy and therefore onset-like events.

#### 24. Why rolling normalization?

Mastering levels and genres differ. Percentile-based recent context maps features relative to the current material, so a quiet track can still create a strong relative response.

#### 25. How is beat timing estimated?

Strong onsets separated by at least 240 ms become beat candidates. Accepted intervals between 240 ms and 1.1 s are stored, and their median estimates the beat interval. Phase is elapsed time modulo that interval; pulse decays exponentially after an onset.

#### 26. What are the limits of that beat tracker?

It can confuse subdivisions, half/double tempo, syncopation, and weak-onset music. It is a lightweight visual timing heuristic, not a verified beat grid.

#### 27. How is a musical Build different from one loud frame?

Build uses longer energy/high/onset trends and candidate dwell. Impact uses a separate jump/onset score with cooldown. This separates sustained direction from a transient.

#### 28. Why separate audio freshness from feature values?

The last feature values can remain numerically high after capture stops. A timestamp-derived reactivity factor fades those values toward ambient so the visual cannot freeze on stale energy.

### Scene direction and graphics

#### 29. Why not choose a new shader scene on every beat?

That would flicker and destroy visual identity. Phrases/state choose stable macro scenes; beats animate parameters inside the scene. Minimum dwell and crossfade preserve continuity.

#### 30. How many scenes can render at once?

One base normally. During a transition, exactly two bases—the outgoing and incoming—render with normalized weights. Up to two compatible modifiers can also affect the base, but modifiers are not additional base scenes.

#### 31. How is scene selection deterministic but varied?

The director hashes a session seed with phrase/track/index or fallback-state keys. The same seed/key/history produces the same plan, while a new session seed creates variation. Bounded history prevents a third consecutive identical primary.

#### 32. Why a full-screen triangle?

It covers the viewport with only three generated vertices and no vertex buffer. The fragment shader procedurally computes every pixel, which is ideal for these abstract effects.

#### 33. What are uniforms?

They are a small CPU-written parameter block read by every shader invocation for a frame. They carry resolution, time, music features, scene IDs/weights, palettes, and modifier strengths.

#### 34. Why smooth values on the renderer too?

Analysis values can jump, and user settings can change abruptly. Time-based attack/release smoothing makes motion readable and less frame-rate dependent while retaining quick transient response.

#### 35. How do you prevent excessive brightness?

Flash is opt-in and capped/cooldown-limited. Scene normalization lowers brightness under modifier load. The shader applies a luminance budget before tone mapping.

#### 36. What is the difference between GLSL and WGSL here?

GLSL runs the WebGL browser preview. WGSL runs the native production `wgpu` renderer. They share concepts but are separate shader implementations; only WGSL gets live native audio features.

### Reliability and diagnostics

#### 37. What does transactional startup mean?

The app creates a hidden window and workers, but does not publish/show a live session until the renderer presents its first valid frame. Any earlier error signals workers, joins them with bounds, restores window state, closes it, and returns an error to the surviving controller.

#### 38. Why wait for first present instead of pipeline creation?

An adapter/device/pipeline can succeed while surface acquisition or presentation still fails. First present proves the actual selected window/display path accepted a frame.

#### 39. How does audio failure differ from renderer failure?

Audio failure is recoverable: reactivity fades and visuals continue ambient while capture reconnects. A renderer/device failure means frames cannot be safely shown, so it records the reason and stops/cleans the session.

#### 40. How are unexpected exits diagnosed?

Structured stages update a flushed session marker. If the next run sees that the prior marker remained InProgress, it reconstructs a report containing the last durable stage and `UNEXPECTED_PROCESS_EXIT`, which can then be correlated with OS crash logs.

#### 41. Why stable diagnostic codes?

Human messages can be edited or localized. Stable codes support tests, searching, support playbooks, and machine processing without brittle text matching.

#### 42. Why are diagnostics independent of live Start?

They can test bounded subcomponents without full-screen risk, produce a report even on partial failure, and avoid mutating the live manager’s session state.

#### 43. What is graceful degradation in this project?

Stale audio fades to quiet ambient motion, process-route failure can become an explicit output fallback, and a software GPU adapter may be attempted. Degradation is reported; it is not presented as the preferred route succeeding.

### Testing and improvement

#### 44. What does CI prove?

It proves frontend static/build checks and that Windows/macOS code compiles, lints, tests, and bundles on CI runners. It does not prove actual Rekordbox audio, permissions, latency, or every GPU/display driver path.

#### 45. What would you test next?

I would prioritize a hardware matrix: Windows builds and Rekordbox versions, process vs ASIO/fallback, macOS permission lifecycle, start/stop loops, process restart, silence, device/display disconnect, sleep/wake, emergency exits, representative GPUs, and an event-length soak test measuring memory and latency.

#### 46. What performance work would you consider?

Profile before changing architecture. Likely candidates are rolling percentile sorts, per-analysis `Vec` collection from `VecDeque`, settings write frequency, shader cost at high resolutions, and scheduling/present mode. Add measurements so optimizations preserve visual behavior.

#### 47. What security/privacy risks matter?

Native capture permissions and route transparency matter. Keep audio memory-only, avoid logging identifying device/track/library data, constrain Tauri commands/CSP, validate settings/formats, keep FFI boundaries small, and sign/notarize releases before broad distribution.

#### 48. What is the strongest design decision?

A good answer is the combination of bounded latest-state processing and transactional first-frame startup. It directly matches the live-show requirement: freshness over history, and no visible half-started output.

#### 49. What is the largest current product limitation?

There is no verified live Rekordbox phrase/deck/playhead provider and no completed evidence matrix on target DJ hardware. The current audio-inferred direction is useful, but it must not be described as native Rekordbox phrase integration.

#### 50. How would you add a future documented Rekordbox provider?

Implement the provider behind the `PlaybackContext`/`PhraseProvider` boundary, require stable track/deck identity, version-gate the protocol, timestamp every update, fail closed on unknown versions, preserve provenance, and route stale/error cases back to audio inference without stopping capture or rendering.

### Behavioral/project-story prompts

Use these as structures, not as permission to claim work you did not personally perform. A strong story states the problem, constraint, decision, evidence, and remaining limitation.

#### Story A: Making startup transactional

- **Problem:** Window creation alone does not prove the GPU can render to the selected display. Revealing early can show broken or blank output.
- **Constraint:** The controller must survive so the user can recover.
- **Decision:** Create/configure the performance window hidden, start bounded workers, wait for a real first present, then reveal. Roll back on every earlier failure.
- **Evidence:** Lifecycle and rollback unit tests, explicit stage diagnostics, a 12-second readiness bound, and a hidden-surface diagnostic.
- **Honest limitation:** CI does not cover every real GPU/display driver.

#### Story B: Keeping native audio cleanup correct

- **Problem:** WASAPI requires every acquired packet to be released exactly once across success, cancellation, and conversion failure.
- **Constraint:** Native early-return paths are easy to miss.
- **Decision:** Wrap the packet in an RAII type with a one-time release gate and a `Drop` fallback; use reverse-order `Drop` cleanup for Core Audio sessions too.
- **Evidence:** The exactly-once release-gate test and bounded worker cleanup tests.
- **Honest limitation:** FFI still contains manually verified `unsafe` invariants.

#### Story C: Avoiding a misleading “connected” status

- **Problem:** A process can exist while capture fails; a client can initialize while packets are silent; audio says nothing about phrase metadata.
- **Constraint:** The user needs actionable event-night diagnosis.
- **Decision:** Model process detection, capture initialization, packets, non-silent signal, reactive readiness, route, and phrase provenance as separate facts.
- **Evidence:** UI tiles, diagnostic schema, stable failure codes, and a test that keeps process detection separate from sample flow.
- **Honest limitation:** Only real hardware can validate every routing configuration.

#### Story D: Prioritizing freshness over completeness

- **Problem:** If analysis falls behind, replaying queued beats makes visuals visibly late.
- **Constraint:** Memory and latency must stay bounded for long sessions.
- **Decision:** Use an overwrite-oldest PCM queue, discard excessive backlog, and render latest snapshots instead of an event queue.
- **Evidence:** Ring-bound tests, the 24,000-sample backlog rule, and stale-audio fade tests.
- **Honest limitation:** This design is appropriate for visuals but deliberately unsuitable for recording.

#### Story E: Diagnosing native exits without a returned error

- **Problem:** A whole-process native exit may occur before normal error handling or UI reporting.
- **Constraint:** Logs must remain bounded and avoid private audio/library data.
- **Decision:** Write structured stage events and synchronously replace a durable in-progress marker; reconstruct the previous failure stage on next launch.
- **Evidence:** rotation, stage ordering, schema compatibility, and unexpected-exit reconstruction tests.
- **Honest limitation:** The last durable stage narrows the search but does not replace an OS crash dump or Event Viewer record.

---

## 21. “Explain this code” mini drills

### Drill 1: `Arc::clone`

If asked why the code writes `Arc::clone(&stop)` instead of just using `stop`, say:

> Moving `stop` would give ownership to one worker and make it unavailable to the manager. Cloning an `Arc` increments the reference count so both own the same atomic flag. It clones the smart pointer, not the underlying state.

### Drill 2: `unwrap_or_else(|poisoned| poisoned.into_inner())`

> Standard locks become poisoned if a holder panics. This code deliberately recovers the inner status/snapshot so supervision and cleanup can continue, instead of turning one worker panic into cascading panics.

### Drill 3: overwrite-oldest queue

> `push` first tries to enqueue. If full, it pops one oldest value, increments a drop metric, and pushes the new sample. Capacity stays fixed and current audio wins.

### Drill 4: asymmetric envelope

> It picks a shorter time constant when target is above current and a longer one when it is below. That makes attacks responsive and releases smooth. The exponential formula uses frame delta, so it behaves consistently across variable frame rates.

### Drill 5: readiness channel

> The manager must know whether a real first frame presented. A one-slot channel carries exactly that one-time result from the renderer worker and supports a timeout/disconnect error without sharing partially initialized GPU objects.

### Drill 6: `Drop` on a WASAPI packet

> Native WASAPI requires every acquired buffer to be released exactly once. The wrapper has a release gate; explicit release and `Drop` both use it. This protects success, conversion error, cancellation, and early-return paths.

### Drill 7: `#[cfg(target_os = ...)]`

> Conditional compilation excludes irrelevant native backends and dependencies. Windows builds compile WASAPI; macOS builds compile Core Audio; other desktop platforms compile an honest unsupported stub.

---

## 22. Suggested reading path through the code

### Pass 1: product/control flow

1. `README.md`
2. `app/src/visuals/types.ts`
3. `app/src/control/transport.ts`
4. `src-tauri/src/commands.rs`
5. `src-tauri/src/lib.rs`
6. `src-tauri/src/display/mod.rs`

Goal: follow Start, Stop, settings, status, and the session lifecycle.

### Pass 2: audio and analysis

1. `src-tauri/src/audio/types.rs`
2. `src-tauri/src/audio/ring_buffer.rs`
3. `src-tauri/src/audio/format.rs`
4. `src-tauri/src/analysis/types.rs`
5. `src-tauri/src/analysis/fft.rs`
6. `src-tauri/src/analysis/rhythm.rs`
7. `src-tauri/src/analysis/musical_state.rs`
8. `src-tauri/src/analysis/mod.rs`

Then read the platform backend for the OS you care about.

### Pass 3: direction and graphics

1. `src-tauri/src/phrase/mod.rs`
2. `src-tauri/src/visuals/state.rs`
3. `src-tauri/src/visuals/director.rs`
4. `src-tauri/src/visuals/palette.rs`
5. `src-tauri/src/visuals/renderer.rs`
6. `src-tauri/shaders/performance.wgsl`

Goal: explain which layer chooses identity and which layer supplies fast animation.

### Pass 4: reliability

1. `src-tauri/src/diagnostics.rs`
2. `src-tauri/src/diagnostic_runner.rs`
3. `docs/reliability.md`
4. the unit tests at the bottom of every module.

Tests are often the clearest executable statement of an invariant.

---

## 23. Hands-on exercises

### Beginner

1. Run the browser app and change every setting. Explain why audio never reacts.
2. Find the Rust default for every TypeScript default setting and verify the names match through Serde camelCase.
3. Trace `controlTransport.start` to the Rust `PerformanceManager::start` call.
4. Find every possible `OutputMode` and explain how the shader receives Black.
5. Change no code: use tests to identify which module owns each invariant.

### Intermediate

1. Calculate ring capacity in samples/bytes for 5, 10, and 30 seconds.
2. Calculate FFT duration, hop duration, and bin width.
3. Draw the lifecycle state machine from code without looking at the docs.
4. Write down every owner/clone of `stop_requested` during a live session.
5. Follow one `f32` sample from a native packet to the shader’s bass-driven deformation.
6. Explain why selected-output silence does not trigger the same route behavior as automatic fallback silence.
7. Add a hypothetical new `VisualFamily` on paper and list every Rust/WGSL/GLSL/test location that must change.

### Advanced/interview preparation

1. Propose a benchmark for end-to-end audio-to-photon latency and identify instrumentation points.
2. Design a streaming percentile replacement and state how you would prove visual behavior remains acceptable.
3. Design integration tests with a deterministic synthetic audio source without weakening the real platform backends.
4. Propose an event-based frontend status channel while preserving renderer independence.
5. Threat-model the native FFI and Tauri command boundary.
6. Explain how you would collect a Windows minidump and correlate it with report/session IDs.
7. Design a versioned future phrase provider and its stale/failure tests.

---

## 24. Study plan

### Week 1: foundations

- Day 1: product, repository, frontend/native distinction.
- Day 2: Rust variables, structs, enums, `Option`, `Result`, `match`.
- Day 3: ownership, borrowing, `String`/`&str`, collections.
- Day 4: traits, modules, Serde, conditional compilation.
- Day 5: `Arc`, locks, atomics, threads, channels, RAII.
- Day 6: TypeScript/React/Tauri bridge.
- Day 7: explain app launch and Start/Stop aloud.

### Week 2: signal and visuals

- Day 8: PCM, sample rate, channels, RMS, resampling.
- Day 9: FFT, Hann window, bands, spectral flux.
- Day 10: normalization, rhythm, state hysteresis.
- Day 11: phrase inference and honest limitations.
- Day 12: scene director, crossfades, modifiers, palettes.
- Day 13: `wgpu`, surfaces, pipeline, uniforms, shader.
- Day 14: trace one frame end to end.

### Week 3: production engineering

- Day 15: Windows audio route.
- Day 16: macOS audio route and FFI/RAII.
- Day 17: diagnostics and crash reconstruction.
- Day 18: failure cases and graceful degradation.
- Day 19: tests, CI, packaging, hardware gaps.
- Day 20: tradeoffs and future improvements.
- Day 21: conduct a mock interview using all 50 questions.

---

## 25. Final cheat sheet

### Numbers worth knowing

- 48 kHz mono internal audio.
- 5–30 second ring; 10 seconds default.
- 2,048-sample FFT; 960-sample/20 ms hop.
- About 23.44 Hz per FFT bin.
- 24,000-sample/0.5-second backlog threshold.
- Audio reaction full for 300 ms, fades to ambient by 2 seconds.
- Process retry: 3 seconds initially, 5 seconds after loss.
- Default fallback preferred-route retry: 30 seconds of silence.
- State dwell: roughly 0.55–0.9 seconds.
- Impact cooldown: 1.25 seconds; hold/release about 0.42 seconds.
- Phrase observations: every 250 ms, max 256/about 64 seconds.
- Phrase dwell: 8–28 seconds.
- Scene minimum dwell: 12 seconds.
- At most two base families during a normalized crossfade.
- At most two compatible modifiers.
- Renderer target: about 60 Hz.
- Live renderer readiness: 12 seconds.
- Main-thread surface creation: 5 seconds.
- Worker join: 2 seconds per worker.
- Log rotation: about 1 MB with three rotated files.
- 44 Rust tests passing when this guide was written.

### The six distinctions interviewers may test

1. Browser preview vs native production renderer.
2. Process detected vs capture initialized vs non-silent/reactive.
3. Audio-inferred phrase vs actual Rekordbox metadata.
4. Buffer capacity vs steady-state latency.
5. One base plus modifiers vs multiple independent scenes.
6. Audio recovery/ambient degradation vs renderer failure/session stop.

### The project’s thesis

The deepest architectural idea is:

> In a live visual system, bounded freshness, explicit provenance, and recoverable lifecycle state matter more than retaining every historical event.

If you can defend that sentence using the ring buffer, latest snapshots, audio-freshness fade, scene dwell, transactional first present, stable diagnostics, and bounded cleanup, you understand the project at an interview-ready architectural level.

---

## 26. September 2026 update supplement

This supplement was added on **September 2, 2026**. Sections 1–25 have deliberately been preserved rather than rewritten. Read them for the foundations, then use Sections 26–35 as the authority whenever an older statement conflicts with a current one.

The repository changed in three important ways after the original guide was written:

1. Every production illusion gained more visible, frequency-specific musical response and musically timed scene changes.
2. Full-screen presentation was changed to keep the physical surface at the exact window size while rendering expensive shader work into a separate HD-capped target.
3. Installation instructions were consolidated into `README.md`, and temporary handoff briefs and transfer tooling were removed.

Some facts in the original guide were also already behind the implementation. This table is the quickest correction map.

| Earlier statement | Current implementation |
| --- | --- |
| Windows prefers native Rekordbox process loopback | Normal Windows operation discovers Rekordbox but captures the stable default Windows output. Native process loopback is an isolated developer opt-in. |
| macOS only exposes a Rekordbox process tap | macOS exposes Rekordbox-only audio, all outgoing system audio, and detected microphone/line-in inputs. |
| Eight base visual families | Auto can select any of 26 distinct analytic illusion families. |
| Manual visual families are selectable | Auto is the only product direction mode. Old style variants remain in serialized types for compatibility, but Rust sanitization forces Auto. |
| Scene minimum dwell is 12 seconds | The current minimum dwell is 8 seconds, with additional timed Auto shuffles aligned to musical boundaries. |
| Audio mainly changes generic speed and intensity | A proportional continuous drive plus four transient lanes independently change geometry, depth, density, and color. |
| High-resolution output uses a smaller presentation surface | The presentation surface always matches the physical window; only the internal shader target is capped to an HD pixel budget and then linearly upscaled. |
| 44 Rust tests | The current suite has 52 passing Rust tests. |
| Windows transfer guide, implementation briefs, and transfer-zip script are part of the repository | Installation now lives in `README.md`; the temporary briefs and transfer archive script were removed. `scripts/build-windows.ps1` remains. |

One documentation caveat matters when studying: `docs/architecture.md` still contains an older 45 FPS/2560×1440 renderer description. The current renderer source, `README.md`, and `docs/visual-language.md` describe the implemented 60 FPS/full-surface/HD-internal path. In an interview, explain the current source behavior and openly identify the documentation drift if asked.

### Updated one-sentence architecture

PulseBridge is a Tauri desktop app whose React controller configures a native Rust pipeline that safely captures live audio, reduces it to bounded musical state, directs one of 28 Auto-selected analytic illusions, and renders a tear-free full-screen result through a 60 FPS `wgpu` surface.

### Updated 90-second interview pitch

> PulseBridge turns Rekordbox-related audio into native second-screen visuals. The React webview is only a controller and ambient preview; the real-time path is Rust. On Windows, the normal Rekordbox-aware source discovers the process but deliberately uses stable output loopback because process-specific activation caused native heap corruption on affected hardware. On macOS 14.2+, users can choose a private Rekordbox process tap, a global system-audio tap, or a microphone/line-in input. Samples are normalized to mono 48 kHz and sent through a bounded overwriting queue to FFT, rhythm, musical-state, and phrase-inference workers. The renderer derives a continuous audio-drive envelope and separate bass, mid, high, and energy-rise events. A deterministic-but-shuffled director selects among 28 illusions, avoids recent repetition, and waits for musical boundaries before transitions. Production output uses a physical-size FIFO surface at 60 FPS, an HD-capped internal shader target, linear upscaling, and derivative-aware line filtering. Transactional startup, first-frame readiness, explicit route facts, ambient degradation, and durable diagnostics make failures visible and recoverable.

---

## 27. Updated audio capture and safety model

### Why the Windows default changed

The ideal semantic route is “only Rekordbox.” Windows provides process-loopback activation for this, but the affected test machine repeatedly terminated with `STATUS_HEAP_CORRUPTION` before the asynchronous activation callback returned. That is not a normal Rust `Result` that can be caught; it is native memory corruption capable of killing the process.

The production decision is therefore evidence-based:

- keep deterministic Rekordbox process discovery;
- report whether Rekordbox is detected;
- route normal `process:auto` operation through proven default-output WASAPI loopback;
- expose exact route and fallback facts in status and diagnostics;
- retain process-specific code only for isolated developer validation.

This is a useful interview example of choosing reliability over a more elegant feature. The label “Rekordbox-aware” refers to discovery and product intent; it does **not** falsely claim that the normal Windows audio packets come from a process-isolated stream.

### Current Windows route matrix

| Selected source | Normal behavior | Reported route |
| --- | --- | --- |
| `process:auto` | Discover Rekordbox, then use default Windows render-output loopback in safe mode | `defaultOutputFallback` |
| `device:<id>` | Capture the explicitly selected Windows output device | `selectedOutput` |
| `process:auto` with developer opt-in | On a supported build, attempt each deterministic Rekordbox PID, then fall back to output if unavailable | `rekordboxProcess` or `defaultOutputFallback` |

The opt-in environment variable is `PULSEBRIDGE_EXPERIMENTAL_PROCESS_LOOPBACK`. Values `1`, `true`, or `yes` enable it. The native API also requires Windows build 20348 or newer. It is for controlled hardware tests, not normal party use.

If Rekordbox uses ASIO, its master audio may bypass the ordinary Windows output. The practical solution is to enable PC MASTER OUT and select the Windows output that actually carries the master signal. Process detection alone cannot prove sample flow.

### Current macOS route matrix

| Source ID | Meaning | Permission/requirement |
| --- | --- | --- |
| `process:auto` | Private Core Audio tap for the discovered Rekordbox process | macOS 14.2+ and Screen & System Audio Recording permission |
| `output:default` | Core Audio tap for all outgoing system audio | macOS 14.2+ and Screen & System Audio Recording permission |
| `input:<stable-device-id>` | Selected microphone, line-in, or virtual input through `cpal` | Microphone permission; Rekordbox is not required |

Input devices are enumerated separately, sorted with the default first, and converted from their negotiated sample format into the same mono 48 kHz analysis contract. If an input disappears or a tap ends, the worker enters recovery and retries instead of pretending the old stream is still active.

### Five separate connection facts

Do not collapse these into one vague “connected” boolean:

1. **Process detected:** the OS can see Rekordbox.
2. **Capture initialized:** an audio client/tap/stream was created.
3. **Packets received:** callbacks or WASAPI reads are arriving.
4. **Non-silent samples received:** the packets contain measurable signal.
5. **Reactive ready:** enough current live signal has reached analysis for the visuals to respond.

A system can satisfy the first three and still fail the fourth because the selected route is silent. It can satisfy capture without process detection when a macOS input is intentionally selected. Keeping the facts independent makes troubleshooting truthful.

### Updated interview answer: “Why not always capture the Rekordbox process?”

> Process isolation is desirable, but a real affected Windows machine showed native heap corruption during asynchronous process-loopback activation. Because that failure can terminate the entire app outside Rust’s recoverable error model, the shipping route prioritizes stable output loopback and reports the actual route. Process discovery remains useful, and the isolated API remains behind an explicit developer flag for hardware validation. On macOS 14.2+, private process taps are stable enough to remain a supported user route.

---

## 28. Auto-only direction and the 28-illusion library

### The complete production family list

The numeric IDs are part of the Rust-to-WGSL contract. `VisualFamily` supplies the ID and the WGSL `visual_family` switch dispatches the same number.

| IDs | Families |
| --- | --- |
| 0–4 | Techno Laser Grid, Moiré Rings, Infinite Checker, Neon Lattice, Twisted Stripes |
| 5–9 | Rotating Snakes, Hyperbolic Tunnel, Chromatic Maze, Vortex Chevron, Glass Orbit |
| 10–14 | Sine Interference, Impossible Cubes, Polar Fan, Gravity Lens, Ribbon Wormhole |
| 15–19 | Quantum Weave, Fractal Compass, Liquid Circuit, Spinning Alien, Prism Vortex |
| 20–25 | Diamond Drift, Orbital Mesh, Helix Portal, Radial Escalator, Electric Topography, Event Horizon |
| 26–27 | Kinetic Bars, Bulging Checker |

The production contract is:

- exactly one family normally renders;
- exactly two may render during an outgoing/incoming crossfade;
- their crossfade weights are normalized;
- at most two compatible modifiers may sit on top;
- a modifier never secretly invokes a second unrelated base shader.

### Why old style variants still exist

Rust and TypeScript still define `Fluid`, `Waves`, `Pulse`, `Tunnel`, and `Burst`. This preserves compatibility with settings that an older build may have serialized. However, `VisualSettings::sanitized` always sets `style = VisualStyle::Auto`, and the controller no longer offers style buttons.

That is a migration pattern: accept the old data shape at the boundary, normalize it into the new invariant, and keep the runtime simpler. Deleting variants immediately could break deserialization of an existing `visual-settings.json`.

### How Auto chooses without feeling random or repetitive

The result is deterministic for the same session seed and direction key, but it changes over time:

1. Fresh playback context contributes phrase kind, stable track identity, and segment index. If it is missing or older than five seconds, inferred musical state supplies the direction.
2. A timed shuffle counter is mixed into the direction key.
3. `choose_primary` starts at a seed-derived point in the 28-item array.
4. It searches for a family absent from the last four selections.
5. The full recent-history deque remains bounded to eight entries.
6. A new family still respects the eight-second minimum dwell and crossfades instead of cutting abruptly.

All phrase kinds currently draw from all 28 families. Phrase/state still matters because it controls the key, palette, motion/detail/density/brightness budgets, transition duration, modifier preference, and the reason reported by the director.

### Musically aligned shuffling

Auto becomes eligible to shuffle on an intensity-dependent cadence:

| Profile | Base shuffle interval | Quiet fallback deadline |
| --- | ---: | ---: |
| Chill | 18 s | 28.8 s |
| Balanced | 14 s | 22.4 s |
| Wild | 10 s | 16 s |

Once due, the director waits for any of these musical boundaries:

```text
impact > 0.54
OR onset > 0.62 AND energy > 0.34
OR beat_pulse > 0.66 AND energy > 0.58
```

If no boundary occurs, the `1.6 × interval` deadline forces progress. This is the key compromise: transitions feel musical when a boundary is available but cannot become stuck during quiet or poorly detected material.

### Modifier timing

Chill clears normal modifiers. Balanced rotates them on a 16-second cadence; Wild uses seven seconds. A due modifier waits for impact above 0.42, onset above 0.54, or a beat above 0.58 while energy exceeds 0.48. Its bounded fallback is `1.5 ×` the cadence. A separate short Impact Bloom may still be added when impact crosses the profile threshold, subject to compatibility and the two-slot cap.

The eight modifier IDs remain Palette Drift, Beat Zoom, Bass Warp, High Sparkle, Echo Trails, Mirror Fold, Chromatic Split, and Impact Bloom.

---

## 29. Proportional drive and four reactive lanes

### The design problem

A single “loudness makes animation faster” mapping often looks prerecorded. Two songs at similar average loudness produce nearly the same picture even if one is bass-heavy and the other has sharp hats or moving vocals.

The current design has two layers:

- **continuous drive** answers “how active is the music overall?”;
- **transient lanes** answer “what kind of musical change just happened?”

### Continuous drive

The smoothed target begins with weighted tonal energy:

```text
tonal = 0.48*energy + 0.22*bass + 0.12*mids + 0.18*highs
continuous = clamp((tonal - 0.10) / 0.78, 0, 1)
drive_target = clamp(
    0.78*continuous + 0.12*beat + 0.10*onset + 0.20*impact,
    0, 1
) * live_audio_reactivity
```

Its envelope attacks in about 55 ms and releases over about 340 ms. This lets activity arrive quickly but settle without flicker.

The selected intensity profile is a **ceiling**, not a forced level:

| Profile | Drive ceiling |
| --- | ---: |
| Chill | 0.48 |
| Balanced | 0.74 |
| Wild | 1.00 |

Thus Wild permits the complete range while quiet music can still remain quiet. The Music reactivity control and intensity multipliers then bound how strongly drive and events influence the uniform values.

### The four transient lanes

Changes are measured against the previous smoothed values **before** those values are updated. This matters: comparing a new sample with an already-updated smoother would erase much of the transient.

| Lane | Main evidence | Envelope | Visible role |
| --- | --- | --- | --- |
| `bass_hit` | Beat, onset accent, positive bass rise, positive sub rise | 12 ms attack / 200 ms release | Radial geometry wave, depth jump, zoom |
| `mid_motion` | Normalized mid level, absolute mid change, onset accent | 35 ms / 280 ms | Two-axis geometry bends and colored ribs |
| `high_hit` | Positive high rise, onset accent, normalized high level | 9 ms / 140 ms | Slices, radial color shards, palette shift |
| `energy_rise` | Positive whole-band energy rise, onset accent, impact | 12 ms / 260 ms | Scale/depth change, density, brightness |

An onset accent begins only above 0.22 and is normalized across the remaining range. The visual beat is the maximum of the analyzed beat pulse and an onset/bass-derived inferred beat. This helps percussive content create a crisp visible event even when the rhythm tracker has not formed a strong periodic estimate.

When live audio becomes stale, `AnalysisSnapshot::visual_input` holds full reactivity for 300 ms, fades it to zero by two seconds, and changes the state to Quiet near zero. Because all four transient targets multiply by this live-audio reactivity, ambient fallback cannot invent bass hits or shards.

### How the renderer uses the lanes

The Rust renderer packs `[bass_hit, mid_motion, high_hit, energy_rise]` into a `reactive` uniform. The WGSL shader applies these **before** choosing a family, so every one of the 26 bases receives the same core geometry response. It then adds family-independent fronts, ribs, shards, spectral tint, and luminance changes after the base is evaluated.

This placement is why the update applies consistently across the whole library. If the effects lived inside only a few family functions, changing scenes could make the same music suddenly stop reacting.

### Updated interview answer: “Why both drive and events?”

> Drive represents sustained musical activity, so motion scales proportionally instead of jumping between static presets. The four short-lived lanes preserve instrumentation: bass changes radius and depth, mids bend the field, highs create spatial and color detail, and energy rises change scale and light. Separate attack/release envelopes make those cues readable and prevent one noisy feature from controlling everything.

---

## 30. Current native rendering and presentation path

### Physical surface versus internal render target

The renderer now owns two differently sized images:

```text
WGSL full-screen triangle
        |
RGBA8 sRGB internal texture
(physical size up to an HD pixel budget)
        |
linear TextureBlitter copy
        |
surface texture exactly matching the window/display
        |
FIFO present
```

The distinction fixes a subtle full-screen problem. A compositor expects the swapchain/surface texture to describe the whole physical window. Making the surface itself smaller on a 4K display can lead to platform-dependent partial coverage or unusual scaling. Keeping the surface at physical size gives presentation the correct geometry.

The expensive analytic shader does not need to run at four times the pixels, so it renders into a separate `Rgba8UnormSrgb` texture. `performance_render_size` preserves the aspect ratio and caps the internal area to `1920 × 1080 = 2,073,600` pixels. A 1920×1080 window stays 1920×1080; 3840×2160 becomes 1920×1080 internally. Other aspect ratios scale proportionally to the same pixel budget rather than being stretched to 16:9.

The texture blitter samples that internal texture linearly into the full physical surface. This gives full-screen coverage, predictable performance, and much less shader work on 4K output.

### Frame pacing and presentation

- `FRAME_INTERVAL` is 16,666,667 nanoseconds, approximately 60 FPS.
- The surface uses `PresentMode::Fifo`, the broadly supported tear-free/vsync-style queue.
- `desired_maximum_frame_latency` is one to reduce queued frames where the backend honors it.
- If rendering finishes early, the worker sleeps until the next target.
- If it falls behind, it resets the schedule to the current time instead of burst-rendering missed frames.
- Surface loss recreates and resizes both the physical surface and internal target.

The sleep loop is a CPU-side target, while FIFO presentation is the display-facing synchronization. They complement each other but do not guarantee that every physical monitor refresh is exactly 60 Hz.

### Why `fwidth` appears in `ridge` and `glow`

Analytic illusions generate thin periodic lines from mathematical phase values. On a distant or high-frequency region, the phase can change by more than a line width from one pixel to the next. A pixel grid cannot resolve that detail, so naïve sampling aliases into blocks, crawling patterns, or unstable moiré.

In a fragment shader, `fwidth(value)` estimates how much `value` changes across neighboring screen fragments. The shared `ridge` and `glow` helpers use it to fade a feature once the phase changes too quickly to resolve. The high-frequency shard rays use the same derivative-aware helpers.

This is not ordinary object-space smoothing. It is **screen-space level-of-detail filtering**: retain a line when pixels can represent it and suppress it before undersampling creates artifacts.

### Why the controller preview pauses

The browser preview is a separate WebGL context. While native output is live, `PerformanceCanvas` receives `paused=true`; its effect cleanup cancels animation and deletes the WebGL vertex array/program. This avoids making the laptop render two animated GPU paths while Rekordbox and the native output are already active.

The preview also keeps its reactive uniform at zero. It demonstrates palette and general appearance, not live capture. This prevents a polished browser animation from being mistaken for proof that the native audio pipeline works.

---

## 31. Updated repository and installation knowledge

### Current source map changes

The original repository table names several handoff artifacts that no longer exist. The current rule is simpler:

- `README.md` is the installation and primary command authority;
- `scripts/build-windows.ps1` is the supported Windows verification/installer path;
- `docs/development.md` holds development and hardware-validation boundaries;
- the former Windows transfer guide, two temporary implementation briefs, and `scripts/make-transfer-zip.sh` were removed;
- this study guide has now been restored because it is a learning artifact, not a temporary implementation handoff.

Do not restore or cite the removed files in a new setup procedure.

### Current prerequisites

- Node.js 22.12 or newer.
- Rust 1.87 or newer.
- macOS: Apple silicon is the currently documented build target, plus Xcode Command Line Tools; full Xcode is needed for DMG tooling.
- Windows: the PowerShell build script installs missing prerequisites when possible and produces an unsigned NSIS installer under `transfer-ready`.

The frontend browser command is `npm run dev`; it does not test native audio. Native development is `npm run tauri -- dev`. The normal verification set remains lint, typecheck, production frontend build, Rust formatting, Clippy with warnings denied, and Rust tests.

### Permission model to remember

- macOS Rekordbox/system taps request Screen & System Audio Recording.
- macOS physical/virtual inputs request Microphone.
- Permissions are requested when the selected route starts, not indiscriminately at launch.
- Windows output capture does not turn process detection into route isolation; the status must say which path is actually active.

---

## 32. Rust and GPU concepts demonstrated by the updates

### Backward-compatible normalization

Keeping old `VisualStyle` enum variants while forcing Auto in `sanitized()` separates **what the app can deserialize** from **what the runtime permits**. This is often safer than changing a persisted schema and hoping every older file migrates cleanly.

### Platform-specific dependencies

`cpal` is declared only under the macOS target dependency section because it is used for selected macOS inputs. WASAPI and Windows API features remain under the Windows target. `cfg`-specific dependencies keep unrelated native backends out of the opposite platform build.

### State versus events

The reactive lanes are stored as envelope state rather than queued as every onset event. The renderer wants the latest amplitude now, not a backlog of historical hits. This follows the same freshness-first philosophy as the PCM overwriting queue and latest analysis snapshot.

### Stable enum-to-shader IDs

`#[repr(u32)]` gives each `VisualFamily` an explicit numeric representation. Rust sends the number as a float in a uniform because GPU uniform layouts are vector-oriented; WGSL rounds it and switches on the corresponding `u32`. Adding or reordering a family requires coordinated Rust enum, candidate array, shader function, shader dispatch, preview representation as appropriate, and tests.

### Offscreen rendering

The internal texture is a GPU resource with both `RENDER_ATTACHMENT` and `TEXTURE_BINDING` usage. The first lets the render pass draw into it; the second lets the blitter sample it. RAII owns the texture, view, blitter, surface, device, and queue so replacements on resize naturally release old GPU handles when dropped.

### Shader derivatives

`fwidth` is available in fragment stages because neighboring fragments execute in small groups. It is unsuitable as a generic CPU formula or vertex-stage assumption. Its use here is tied directly to the current screen resolution, which is exactly what anti-aliasing of procedural lines needs.

---

## 33. Updated end-to-end trace: from deck audio to a 4K display

Use this trace as your main interview narrative.

1. The user chooses a display, source, intensity, palette, flash policy, and advanced controls in React.
2. Rust sanitizes settings, including forcing direction to Auto and clamping numeric ranges.
3. The performance manager starts transactionally and keeps the output hidden until a real first frame is presented.
4. On Windows, `process:auto` discovers Rekordbox but normally starts safe default-output loopback. On macOS, it starts the explicitly selected process tap, system tap, or input device.
5. Native packets are converted, downmixed, and statefully resampled into mono 48 kHz `f32` PCM.
6. The bounded queue overwrites oldest samples under pressure, preserving freshness.
7. The analysis worker consumes overlapping 2048-sample FFT windows every 960 samples and derives energy bands, flux, onset, beat pulse, impact, musical state, and bounded inferred phrase context.
8. The renderer reads only the latest snapshot. Audio freshness supplies a 0–1 reactivity factor; stale input fades to ambient rather than freezing the last hit.
9. `SmoothedVisualState` produces continuous drive plus bass, mid, high, and whole-band rise envelopes.
10. `SceneDirector` combines fresh phrase/state context with the session seed and timed shuffle counter, avoids recent families, honors eight-second dwell, and waits for a musical boundary when possible.
11. Rust packs continuous, transient, scene, palette, modifier, and output-mode values into one uniform structure.
12. WGSL applies common reactive geometry, evaluates one family or a normalized two-family transition, applies compatible modifiers and response overlays, limits luminance, and tone-maps the result.
13. On a 4K display, the shader runs at 1920×1080 internally. A linear GPU blit fills the exact 3840×2160 FIFO surface.
14. The first successful present marks readiness and allows the performance window to become visible.
15. The React preview is torn down while native output runs, but the controller continues polling status and can switch among Reactive, Ambient, and Black or stop the session.

---

## 34. Updated interview questions and model answers

### 1. What is the biggest reliability tradeoff in the current build?

Windows gives up process-isolated audio by default in favor of stable render-output loopback. The change follows observed native heap corruption, while diagnostics preserve process detection and actual-route transparency.

### 2. Does `process:auto` mean the same thing on both platforms?

No. It is a product-level source ID with platform-specific implementation. On macOS it means a private Rekordbox process tap. On normal Windows builds it means Rekordbox-aware discovery plus safe default-output capture; experimental process capture requires an explicit environment flag.

### 3. Why is process detection not enough to show “connected”?

A running process can produce audio through a different route, capture initialization can succeed on a silent endpoint, and packets can contain silence. The app separately reports detection, initialization, packet flow, non-silent signal, and reactive readiness.

### 4. What happens if macOS input hardware disconnects?

The input route reports unavailable sample flow, enters Recovering, waits briefly, and retries the stable selected-device ID. It does not reuse a dead stream or claim readiness.

### 5. Why retain manual-style enum variants when Auto is the only UI mode?

They preserve deserialization compatibility with older settings. Sanitization turns every accepted value into the current Auto invariant before runtime use.

### 6. How can selection be shuffled and deterministic at once?

Pseudo-random choice is derived from a stable session seed, phrase/state key, and counters. Given the same inputs it is reproducible, but counters and music context change the key over time.

### 7. How does the director avoid a predictable short loop?

It draws from all 28 families, searches past any of the last four selected families, stores only eight history entries, observes an eight-second minimum dwell, and mixes in a timed shuffle counter.

### 8. Why wait for a musical boundary, and why also have a timeout?

A cut or crossfade on an impact/onset/strong beat feels intentional. A timeout is necessary because quiet music or weak beat detection could otherwise prevent visual progress indefinitely.

### 9. Why not use a single loudness value for every effect?

It loses instrumentation. Sustained drive conveys overall activity, while separate sub/bass, mid, high, and energy-rise envelopes create visibly different geometry and color responses.

### 10. Why calculate positive rises before smoothing the bands?

The previous smoothed value is the baseline for detecting a new transient. Updating it first would move the baseline toward the new value and weaken the detected rise.

### 11. What do attack and release mean here?

Attack is how quickly an envelope rises toward a stronger target; release is how slowly it falls. Fast attack makes hits immediate, and slower release prevents flicker and gives the eye time to read them.

### 12. How is Wild different from simply setting everything to maximum?

Wild raises the drive ceiling and motion/detail multipliers and shortens direction/modifier cadences. The actual drive remains proportional to audio, so quiet passages still have headroom and contrast.

### 13. Why keep the surface at native display size?

The swapchain image should cover the exact physical window. A smaller surface can produce compositor-dependent partial or incorrectly scaled presentation. Performance scaling belongs in an offscreen internal texture.

### 14. Why cap by pixel area instead of fixed width and height?

An area cap preserves unusual aspect ratios while limiting fragment-shader cost. Fixed 1920×1080 dimensions would distort ultrawide or portrait displays.

### 15. What does the linear blit buy you?

It cheaply samples the HD-capped internal result across the native surface, avoiding nearest-neighbor blocks and avoiding a second evaluation of the expensive procedural shader at 4K.

### 16. What is `fwidth`, conceptually?

It estimates how rapidly a shader expression changes across neighboring pixels. The renderer uses it to fade lines that the current resolution cannot represent, reducing aliasing and blocky procedural detail.

### 17. Why use both a 60 FPS interval and FIFO presentation?

The interval prevents uncontrolled CPU/GPU submission, while FIFO makes presentation tear-free and synchronized to display availability. One governs work scheduling; the other governs swapchain presentation.

### 18. Why destroy the WebGL preview during a live show?

It avoids competing for GPU time with Rekordbox and the native renderer. The preview is informative while stopped but has no role in production audio response.

### 19. Which tests specifically defend the new behavior?

Tests verify 26 unique Auto families, music-boundary shuffle plus bounded fallback, proportional drive, distinct frequency-event lanes, no events without live reactivity, intensity ceilings, valid WGSL, and HD internal sizing for both HD and 4K surfaces.

### 20. What remains unverified by unit tests?

Real Rekordbox routing, permissions, driver behavior, async Windows activation, monitor/compositor behavior, and audio-to-photon latency require hardware integration testing. CI compilation and synthetic tests cannot prove those external systems.

### 21. What would you improve next?

A strong answer is: repair and validate Windows process capture on an affected-device matrix; add deterministic synthetic end-to-end audio fixtures; measure audio-to-photon latency; test unusual DPI/aspect/refresh-rate displays; keep architecture docs synchronized; and pursue signed/notarized releases.

### 22. How would you explain the documentation mismatch professionally?

> The executable source, tests, README, and visual-language document agree on the current 60 FPS full-surface path. One concise architecture page still describes the earlier 45 FPS scaled-surface design. I would treat source and tests as authoritative, document the discrepancy, and update that page in a focused documentation change.

---

## 35. Updated final cheat sheet

### Current facts to memorize

- Rust 2021, minimum Rust 1.87; Node.js 22.12+.
- React 19 controller, Tauri 2 bridge, native Rust workers, `wgpu` 30/WGSL renderer.
- Mono 48 kHz internal audio; bounded 5–30 second overwriting queue; 10 seconds default.
- 2048-sample Hann FFT; 960-sample/20 ms hop; about 23.44 Hz per bin.
- Live reaction remains full for 300 ms after the last audio and fades to zero by two seconds.
- Windows `process:auto` normally uses stable default-output loopback while still reporting Rekordbox discovery.
- Windows process loopback is developer-only via `PULSEBRIDGE_EXPERIMENTAL_PROCESS_LOOPBACK=1` and requires build 20348+.
- macOS 14.2+ supports Rekordbox-only tap, all-system-output tap, and selected microphone/line-in inputs.
- Auto is the only direction mode; old manual variants remain only for serialized compatibility.
- 26 distinct analytic illusion families; history capacity eight; most recent four avoided.
- Eight-second minimum scene dwell.
- Auto shuffle cadence: Chill 18 s, Balanced 14 s, Wild 10 s; musical boundary preferred, `1.6×` fallback.
- Modifier cadence: Balanced 16 s, Wild 7 s; musical trigger preferred, `1.5×` fallback; Chill clears normal modifiers.
- One base normally, two only during a normalized crossfade, no more than two compatible modifiers.
- Drive ceilings: Chill 0.48, Balanced 0.74, Wild 1.0.
- Reactive uniform order: bass hit, mid motion, high hit, energy rise.
- Flash is separately opt-in and defaults Off.
- Production renderer targets 60 FPS with FIFO and desired maximum frame latency one.
- Physical surface always matches the window; internal shader work is capped at 2,073,600 pixels and linearly upscaled.
- Procedural ridge/glow detail uses `fwidth` to fade unresolved screen-space frequency.
- Browser preview is ambient and non-reactive, and it is destroyed while native output runs.
- Current Rust test result: 52 passed, 0 failed.

### Current thesis

> PulseBridge treats correctness as a chain of explicit, bounded facts: the actual capture route is reported, stale audio loses authority, visual state represents the newest music rather than a backlog, scene changes wait for meaningful boundaries without waiting forever, and the swapchain always represents the whole physical display while expensive shader work stays bounded.

If you can explain the engineering reason behind each clause—not merely quote it—you can answer both project-specific questions and broader interview questions about concurrency, native integration, signal processing, backward compatibility, GPU rendering, graceful degradation, and production reliability.
