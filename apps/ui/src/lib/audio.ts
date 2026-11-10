// Optional sound effects and the buzzer component, via WebAudio.

import { app } from './app.svelte';

let ctx: AudioContext | null = null;
const audio = () => (ctx ??= new AudioContext());

const NOTES: Record<string, [number, number][]> = {
  place: [[520, 0.05]],
  wire: [[660, 0.04]],
  delete: [[220, 0.06]],
  toggle: [[440, 0.03]],
  ok: [[660, 0.08], [880, 0.1]],
  fail: [[240, 0.12], [180, 0.16]],
  win: [[523, 0.09], [659, 0.09], [784, 0.09], [1047, 0.18]],
};

export function sfx(name: keyof typeof NOTES) {
  if (!app.sound) return;
  try {
    const a = audio();
    let at = a.currentTime;
    for (const [freq, dur] of NOTES[name]) {
      const osc = a.createOscillator(), gain = a.createGain();
      osc.type = 'triangle';
      osc.frequency.value = freq;
      gain.gain.setValueAtTime(0.06, at);
      gain.gain.exponentialRampToValueAtTime(0.001, at + dur);
      osc.connect(gain).connect(a.destination);
      osc.start(at);
      osc.stop(at + dur);
      at += dur * 0.9;
    }
  } catch {
    /* audio unavailable */
  }
}

const buzzers = new Map<number, OscillatorNode>();

/** Called by the renderer for every visible buzzer. */
export function buzz(id: number, on: boolean, freq: number) {
  const playing = buzzers.get(id);
  if (on && app.sound && !playing) {
    try {
      const a = audio();
      const osc = a.createOscillator(), gain = a.createGain();
      osc.type = 'square';
      osc.frequency.value = freq;
      gain.gain.value = 0.03;
      osc.connect(gain).connect(a.destination);
      osc.start();
      buzzers.set(id, osc);
    } catch {
      /* audio unavailable */
    }
  } else if (playing && (!on || !app.sound)) {
    playing.stop();
    buzzers.delete(id);
  }
}

export function silence() {
  for (const [id, osc] of buzzers) {
    osc.stop();
    buzzers.delete(id);
  }
}
