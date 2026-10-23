//! Dev command for listening to the voice: `lectrice-api speak --text "…" [--voice af_heart] --out file.wav`.

use std::error::Error;
use std::path::PathBuf;
use std::time::Instant;

use lectrice_domain::{AudioEncoder, SpeechRequest, VoiceId};
use lectrice_infra::WavEncoder;

use crate::composition::build_speech_engine;
use crate::config::SpeechConfig;

pub const COMMAND: &str = "speak";

const SERVICE: &str = "devSpeak";
const TEXT_FLAG: &str = "--text";
const VOICE_FLAG: &str = "--voice";
const OUT_FLAG: &str = "--out";
const USAGE: &str = "usage: lectrice-api speak --text \"…\" [--voice af_heart] --out file.wav";

#[derive(Debug, PartialEq, Eq)]
struct SpeakArgs {
    text: String,
    voice: Option<String>,
    out: PathBuf,
}

/// `args` are the command-line arguments after `speak`.
pub fn run(config: &SpeechConfig, args: impl IntoIterator<Item = String>) -> Result<(), Box<dyn Error>> {
    let args = parse(args)?;
    let voice_id = VoiceId::new(args.voice.unwrap_or_else(|| config.default_voice.clone()));

    let started = Instant::now();
    let engine = build_speech_engine(config)?;
    tracing::info!(
        service = SERVICE,
        engine_id = engine.id(),
        voices = engine.voices().len(),
        num_threads = config.num_threads,
        load_seconds = started.elapsed().as_secs_f64(),
        "Speech engine loaded"
    );

    let started = Instant::now();
    let audio = engine.synthesize(SpeechRequest { text: args.text, voice_id: voice_id.clone() })?;
    let synth_seconds = started.elapsed().as_secs_f64();
    let audio_seconds = audio.samples.len() as f64 / f64::from(audio.sample_rate);
    tracing::info!(service = SERVICE, voice_id = %voice_id, sample_rate = audio.sample_rate, audio_seconds, synth_seconds, real_time_factor = audio_seconds / synth_seconds, "Speech synthesized");

    let bytes = WavEncoder.encode(&audio)?;
    std::fs::write(&args.out, &bytes)?;
    tracing::info!(service = SERVICE, out = %args.out.display(), bytes = bytes.len(), "WAV written");
    Ok(())
}

fn parse(args: impl IntoIterator<Item = String>) -> Result<SpeakArgs, String> {
    let (mut text, mut voice, mut out) = (None, None, None);
    let mut args = args.into_iter();
    while let Some(flag) = args.next() {
        let value = args.next().ok_or_else(|| format!("{flag} needs a value; {USAGE}"))?;
        match flag.as_str() {
            TEXT_FLAG => text = Some(value),
            VOICE_FLAG => voice = Some(value),
            OUT_FLAG => out = Some(PathBuf::from(value)),
            _ => return Err(format!("unknown argument {flag}; {USAGE}")),
        }
    }
    match (text, out) {
        (Some(text), Some(out)) => Ok(SpeakArgs { text, voice, out }),
        _ => Err(USAGE.to_owned()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_owned()).collect()
    }

    #[test]
    fn parses_text_voice_and_out() {
        let args = parse(strings(&["--text", "Hello.", "--voice", "bf_emma", "--out", "x.wav"])).unwrap();
        assert_eq!(args, SpeakArgs { text: "Hello.".into(), voice: Some("bf_emma".into()), out: PathBuf::from("x.wav") });
    }

    #[test]
    fn voice_is_optional() {
        assert_eq!(parse(strings(&["--out", "x.wav", "--text", "Hi."])).unwrap().voice, None);
    }

    #[test]
    fn rejects_missing_values_unknown_flags_and_missing_out() {
        assert!(parse(strings(&["--text"])).is_err());
        assert!(parse(strings(&["--text", "Hi.", "--speed", "2", "--out", "x.wav"])).is_err());
        assert!(parse(strings(&["--text", "Hi."])).is_err());
    }
}
