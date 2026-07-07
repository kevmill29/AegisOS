// Audio amplitude source for the sphere. Tries the microphone first (the JARVIS
// fantasy needs the room's sound, not a media element's); if permission is denied
// or no device exists, falls back to a synthetic breathing pulse so the sphere is
// never a dead rock — an idle HUD that visibly "breathes" reads as alive, which
// is the entire point of the design.
export async function createAudioSource() {
  try {
    const stream = await navigator.mediaDevices.getUserMedia({ audio: true });
    const ctx = new AudioContext();
    const source = ctx.createMediaStreamSource(stream);
    const analyser = ctx.createAnalyser();
    analyser.fftSize = 256;
    analyser.smoothingTimeConstant = 0.85;
    source.connect(analyser);
    const bins = new Uint8Array(analyser.frequencyBinCount);

    return {
      kind: 'microphone',
      // RMS over the frequency bins, normalized to roughly 0..1 for typical
      // speech/music levels. The sphere shader clamps anyway, so a hot mic
      // saturates gracefully instead of exploding the geometry.
      getLevel() {
        analyser.getByteFrequencyData(bins);
        let sumSquares = 0;
        for (let i = 0; i < bins.length; i++) {
          const v = bins[i] / 255;
          sumSquares += v * v;
        }
        return Math.min(1, Math.sqrt(sumSquares / bins.length) * 2.5);
      },
    };
  } catch {
    return {
      kind: 'synthetic',
      getLevel() {
        const t = performance.now() / 1000;
        // Two slow sines beating against each other: irregular enough to not
        // look like a metronome, cheap enough to not matter.
        return 0.12 + 0.08 * Math.sin(t * 0.9) * Math.sin(t * 0.23 + 1.7);
      },
    };
  }
}
