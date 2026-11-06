export const encodeWav = (samples: Float32Array, sampleRate: number): Uint8Array<ArrayBuffer> => {
  const bytes = new Uint8Array(44 + samples.length * 2);
  const view = new DataView(bytes.buffer);
  const ascii = (offset: number, s: string) => {
    for (let i = 0; i < s.length; i++) view.setUint8(offset + i, s.charCodeAt(i));
  };
  ascii(0, "RIFF");
  view.setUint32(4, 36 + samples.length * 2, true);
  ascii(8, "WAVE");
  ascii(12, "fmt ");
  view.setUint32(16, 16, true);
  view.setUint16(20, 1, true);
  view.setUint16(22, 1, true);
  view.setUint32(24, sampleRate, true);
  view.setUint32(28, sampleRate * 2, true);
  view.setUint16(32, 2, true);
  view.setUint16(34, 16, true);
  ascii(36, "data");
  view.setUint32(40, samples.length * 2, true);
  for (let i = 0; i < samples.length; i++) {
    const s = Math.max(-1, Math.min(1, samples[i]));
    view.setInt16(44 + i * 2, Math.round(s * 32767), true);
  }
  return bytes;
};

export const downmix = (channels: Float32Array[]): Float32Array => {
  if (channels.length === 1) return channels[0];
  const out = new Float32Array(channels[0].length);
  for (const ch of channels) for (let i = 0; i < out.length; i++) out[i] += ch[i] / channels.length;
  return out;
};

export const toWavBlob = async (input: Blob, maxSeconds = 60): Promise<{ wav: Blob; seconds: number }> => {
  const ctx = new AudioContext({ sampleRate: 48000 });
  try {
    const buffer = await ctx.decodeAudioData(await input.arrayBuffer());
    const channels = Array.from({ length: buffer.numberOfChannels }, (_, i) => buffer.getChannelData(i));
    const mono = downmix(channels).subarray(0, Math.floor(maxSeconds * buffer.sampleRate));
    const wav = encodeWav(mono, buffer.sampleRate);
    return {
      wav: new Blob([wav], { type: "audio/wav" }),
      seconds: mono.length / buffer.sampleRate,
    };
  } finally {
    await ctx.close();
  }
};
