// Sonido de aviso al terminar, sintetizado con Web Audio (sin archivos externos).

let ctx: AudioContext | null = null;

/** WebKit solo permite audio tras una interacción: se prepara con el primer clic. */
export function primeAudio() {
  const prime = () => {
    try {
      ctx ??= new AudioContext();
      if (ctx.state === "suspended") void ctx.resume();
    } catch {
      /* sin audio disponible */
    }
  };
  window.addEventListener("pointerdown", prime, { once: false, capture: true });
  window.addEventListener("keydown", prime, { once: false, capture: true });
}

function tone(c: AudioContext, freq: number, start: number, dur: number, gain = 0.18) {
  const o = c.createOscillator();
  const g = c.createGain();
  o.type = "sine";
  o.frequency.value = freq;
  g.gain.setValueAtTime(0, start);
  g.gain.linearRampToValueAtTime(gain, start + 0.02);
  g.gain.exponentialRampToValueAtTime(0.0001, start + dur);
  o.connect(g).connect(c.destination);
  o.start(start);
  o.stop(start + dur + 0.05);
}

/** Éxito: acorde ascendente. Problema: dos notas descendentes. */
export async function playFinishSound(kind: "success" | "problem") {
  try {
    ctx ??= new AudioContext();
    if (ctx.state === "suspended") await ctx.resume();
    const t = ctx.currentTime + 0.03;
    if (kind === "success") {
      tone(ctx, 659.25, t, 0.35); // Mi
      tone(ctx, 830.61, t + 0.12, 0.35); // Sol#
      tone(ctx, 987.77, t + 0.24, 0.6); // Si
    } else {
      tone(ctx, 523.25, t, 0.35, 0.2);
      tone(ctx, 392.0, t + 0.22, 0.6, 0.2);
    }
  } catch {
    /* sin audio disponible */
  }
}
