const SAMPLE_RATE = 16_000;
const SECONDS_PER_CHAR = 0.06;
const MIN_SECONDS = 0.8;
const TICK_SECONDS = 0.12;
const TICK_HZ = 440;
const TICK_AMPLITUDE = 0.08;
const HEADER_BYTES = 44;

/** Length of the fake audio for a sentence, at 1× speed. */
export const mockSpeechSeconds = (text: string): number => Math.max(MIN_SECONDS, text.length * SECONDS_PER_CHAR);

/** A playable 16-bit mono PCM WAV: a short soft tick, then silence, lasting `seconds`. */
export const makeMockWav = (seconds: number): Blob => {
  const sampleCount = Math.round(seconds * SAMPLE_RATE);
  const dataBytes = sampleCount * 2;
  const view = new DataView(new ArrayBuffer(HEADER_BYTES + dataBytes));
  const writeAscii = (offset: number, text: string) => {
    for (let i = 0; i < text.length; i += 1) view.setUint8(offset + i, text.charCodeAt(i));
  };
  writeAscii(0, 'RIFF');
  view.setUint32(4, 36 + dataBytes, true);
  writeAscii(8, 'WAVE');
  writeAscii(12, 'fmt ');
  view.setUint32(16, 16, true);              // fmt chunk size
  view.setUint16(20, 1, true);               // PCM
  view.setUint16(22, 1, true);               // mono
  view.setUint32(24, SAMPLE_RATE, true);
  view.setUint32(28, SAMPLE_RATE * 2, true); // byte rate
  view.setUint16(32, 2, true);               // block align
  view.setUint16(34, 16, true);              // bits per sample
  writeAscii(36, 'data');
  view.setUint32(40, dataBytes, true);
  const tickSamples = Math.min(sampleCount, Math.round(TICK_SECONDS * SAMPLE_RATE));
  for (let i = 0; i < tickSamples; i += 1) {
    const fade = 1 - i / tickSamples;
    const value = Math.sin((2 * Math.PI * TICK_HZ * i) / SAMPLE_RATE) * TICK_AMPLITUDE * fade;
    view.setInt16(HEADER_BYTES + i * 2, Math.round(value * 0x7fff), true);
  }
  return new Blob([view.buffer], { type: 'audio/wav' });
};
