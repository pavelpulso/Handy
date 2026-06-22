# Handy — Cloud Dictation Fork (Codex + Groq)

> A focused fork of [cjpais/handy](https://github.com/cjpais/handy) that adds two **online, zero-download** transcription backends — **OpenAI Codex Dictation** and **Groq Whisper** — alongside the existing local models, and gets the whole thing building and running on an Intel (x86_64) Mac.
>
> 📖 **Looking for the original project docs?** → [**README.upstream.md**](README.upstream.md)

Handy is an excellent open-source, local-first speech-to-text app. Local models are private and offline, but on modest hardware they are slow and the good ones are multi-gigabyte downloads. I wanted the *option* of fast, accurate cloud transcription without giving up Handy's global-shortcut, paste-anywhere workflow — and without paying twice for an API key I already have.

So I added cloud transcription as first-class models.

---

## What I added

| Model | Backend | Auth | Download |
|-------|---------|------|----------|
| **Codex Dictation** | OpenAI `chatgpt.com/backend-api/transcribe` | Reuses the local Codex CLI login (`~/.codex/auth.json`) | None |
| **Groq Whisper** | Groq `whisper-large-v3-turbo` (OpenAI-compatible) | API key entered in Settings | None |

Both appear in the normal model picker, are selectable per the usual UX, need no model files on disk, and respect Handy's language selection.

### Why it's useful

- **Speed.** End-to-end transcription lands in ~0.5–0.7s on real recordings — faster than local Whisper-large on a CPU/older GPU.
- **Accuracy.** Whisper-large-v3-class quality without a 1.5GB local download or the RAM to run it.
- **Reuse existing credentials.** Codex Dictation piggybacks on the Codex CLI you're already logged into — no extra key, no extra subscription.
- **Still local-first when you want it.** All the original on-device models remain; cloud is purely additive and opt-in.

---

## How it works

Handy dispatches audio (`Vec<f32>` @ 16kHz mono) to a model engine. The clean extension point was to treat "remote" as just another engine type that never touches disk.

**Backend (`src-tauri/`)**
- `managers/model.rs` — two new `EngineType` variants (`CodexDictation`, `Groq`) plus an `is_remote()` helper. Remote models register as always-`is_downloaded`, with no URL and no file, and are skipped by the download/delete/disk-scan logic.
- `managers/remote_transcription.rs` *(new)* — encodes the captured samples to an in-memory WAV (`hound`), then POSTs `multipart/form-data` via `reqwest`. The blocking HTTP call runs on a dedicated thread to stay clear of Tauri's async runtime. Language codes follow the spec (`auto` is omitted; `zh-Hans`/`zh-Hant` collapse to `zh`).
  - *Codex:* reads `tokens.access_token` → `Authorization: Bearer`, `tokens.account_id` → `ChatGPT-Account-Id`, with `originator: codex_desktop` and the Codex Desktop user-agent.
  - *Groq:* standard OpenAI-compatible audio transcription endpoint with the user's key.
- `managers/transcription.rs` — a `LoadedEngine::Remote` variant; "loading" a remote model is a no-op that just records the active provider, and the transcription dispatch returns the remote text directly.
- `settings.rs` / `shortcut/mod.rs` / `lib.rs` — a persisted `groq_api_key` setting and its Tauri command.

**Frontend (`src/`)**
- A `GroqApiKey` settings component, surfaced in the Models screen only when the Groq model is present.
- Model cards hide "delete"/size affordances for remote models (nothing to delete).

The change set is deliberately surgical: cloud is layered onto the existing engine abstraction rather than bolted on beside it.

---

## Getting it to build & run on x86_64 macOS

This was most of the work, and it's the part worth documenting.

1. **`ort` (ONNX Runtime) has no prebuilt for `x86_64-apple-darwin`.** The local ONNX models pull in `ort`, which couldn't download a binary for this target. Fix: switch `ort` to `load-dynamic` and point it at a system ONNX Runtime (`brew install onnxruntime`). The cloud models don't use ONNX at all, so they're unaffected either way.
2. **Missing build tooling.** `cmake` (for `whisper.cpp`) wasn't installed; CMake 4 also needs `CMAKE_POLICY_VERSION_MINIMUM=3.5`.
3. **The real bug: recording silently hung.** The overlay appeared but no audio was ever captured — for *every* model, including the built-in ones. A thread sample showed a deadlock with one thread stuck inside a CoreAudio call while holding an audio mutex, two others blocked behind it. The cause turned out to be the **hardened runtime** on an ad-hoc-signed release build: macOS TCC couldn't validate the microphone entitlement, so the CoreAudio input stream never opened. Re-signing ad-hoc **without** the hardened runtime fixed it instantly.

[`build-custom.sh`](build-custom.sh) captures the whole recipe — build with the right env, strip the hardened runtime, install to `/Applications` — so a rebuild is one command.

---

## Try it

```bash
git clone https://github.com/pavelpulso/Handy && cd Handy
brew install cmake onnxruntime
./build-custom.sh
open /Applications/Handy.app
```

Then pick **Codex Dictation** or **Groq Whisper** in Settings → Models (paste a Groq key for the latter), and dictate anywhere with the global shortcut.

---

*Built on top of [cjpais/handy](https://github.com/cjpais/handy). All credit for Handy itself goes to its authors; this fork only adds the cloud backends and the x86_64 macOS build path. The original project README is preserved at [README.upstream.md](README.upstream.md).*
