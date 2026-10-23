# Lectrice

A free, fully local reader in the spirit of Speechify. Open a PDF and Lectrice cleans it into a
reading view (no running headers, footers, footnote markers or citation brackets; page numbers and
headings are kept as location aids) and reads it aloud with the Kokoro voice, highlighting each
sentence as it goes. Everything runs on your own machine.

- **Backend** (`backend/`): Rust, clean architecture (domain, infra, app, api). PDF extraction with
  PDFium, a rule-based cleaner, Kokoro TTS via sherpa-onnx, an on-disk library, read-ahead that
  scales with playback speed, a bounded audio cache, and MP3/M4B whole-book export via ffmpeg.
- **Frontend** (`frontend/`): Vite + React + TypeScript reader.

## Run it

You need Rust (rustup, stable), g++, Node.js 22+, and ffmpeg for exports. Download the model and
PDFium first (see `backend/README.md`), then:

```bash
cd backend && cargo run --release -p lectrice-api      # http://127.0.0.1:7878
cd frontend && npm install && npm run dev              # http://localhost:5173
```

`npm run dev:mock` runs the frontend with sample data and no backend.

The Kokoro model bundle goes in `models/kokoro-multi-lang-v1_0/`:
<https://github.com/k2-fsa/sherpa-onnx/releases/download/tts-models/kokoro-multi-lang-v1_0.tar.bz2>

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT license](LICENSE-MIT),
at your option.

**Note on builds:** sherpa-onnx 1.13.x statically links espeak-ng, which is GPL-3.0. The source in
this repository is MIT/Apache-2.0, but a compiled `lectrice-api` binary built with sherpa-onnx 1.13.x
includes espeak-ng, so distributing that binary is subject to GPL-3.0. Third-party licenses:
PDFium (BSD-3-Clause, see `backend/vendor/pdfium/`), Kokoro model weights (Apache-2.0).
