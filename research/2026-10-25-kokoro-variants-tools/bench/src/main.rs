//! Scratch bench: loads a Kokoro bundle through Lectrice's core adapter, speaks each line of a text file,
//! prints one line per sentence (index, samples, seconds, clipped count) and writes 16-bit WAVs.
use std::{env, fs, path::PathBuf, time::Instant};

use lectrice_domain::{SpeechEngine, SpeechRequest, VoiceId};
use lectrice_infra::{KokoroSettings, KokoroSpeechEngine, KokoroVariant};

fn main() {
    let args: Vec<String> = env::args().collect();
    let model_dir = PathBuf::from(&args[1]);
    let threads: u32 = args[2].parse().unwrap();
    let text = fs::read_to_string(&args[3]).unwrap();
    let out_dir = args.get(4).map(PathBuf::from);
    let voice = args.get(5).cloned().unwrap_or_else(|| "af_heart".into());

    let start = Instant::now();
    let settings = KokoroSettings { model_dir, variant: if env::var_os("KV_INT8").is_some() { KokoroVariant::Int8 } else { KokoroVariant::Fp32 }, num_threads: threads, provider: "cpu".into() };
    let engine = KokoroSpeechEngine::load(&settings).expect("engine load failed");
    println!("load_s {:.3}", start.elapsed().as_secs_f64());

    let warm = engine.synthesize(SpeechRequest { text: "This is a short warm up line.".into(), voice_id: VoiceId::new(&voice) }).unwrap();
    assert!(!warm.samples.is_empty());

    let (mut total_audio, mut total_time) = (0.0f64, 0.0f64);
    for (i, line) in text.lines().filter(|l| !l.trim().is_empty()).enumerate() {
        let t = Instant::now();
        let audio = engine.synthesize(SpeechRequest { text: line.into(), voice_id: VoiceId::new(&voice) }).unwrap();
        let secs = t.elapsed().as_secs_f64();
        let dur = audio.samples.len() as f64 / f64::from(audio.sample_rate);
        let clipped = audio.samples.iter().filter(|s| s.abs() >= 0.999).count();
        let nonfinite = audio.samples.iter().filter(|s| !s.is_finite()).count();
        let peak = audio.samples.iter().fold(0.0f32, |m, s| m.max(s.abs()));
        total_audio += dur;
        total_time += secs;
        println!("sent {i} samples {} synth_s {secs:.3} rtf {:.3} clipped {clipped} nonfinite {nonfinite} peak {peak:.3}", audio.samples.len(), secs / dur);
        if let Some(dir) = &out_dir {
            fs::create_dir_all(dir).unwrap();
            let spec = hound::WavSpec { channels: 1, sample_rate: audio.sample_rate, bits_per_sample: 32, sample_format: hound::SampleFormat::Float };
            let mut w = hound::WavWriter::create(dir.join(format!("s{i:02}.wav")), spec).unwrap();
            for s in &audio.samples {
                w.write_sample(*s).unwrap();
            }
            w.finalize().unwrap();
        }
    }
    println!("total_audio_s {total_audio:.2} total_synth_s {total_time:.2} rtf {:.3}", total_time / total_audio);
}
