import { useEffect, useRef } from "react";
import { motion, useMotionValue, useSpring, useTransform } from "motion/react";

interface FerrofluidOrbProps {
  volume: number;
  frequencies: Uint8Array | null;
}

function clamp(value: number) {
  return Math.max(0, Math.min(1, value));
}

function averageBand(frequencies: Uint8Array | null, start: number, end: number) {
  if (!frequencies || frequencies.length === 0) return 0;

  const from = Math.max(0, Math.min(frequencies.length - 1, start));
  const to = Math.max(from + 1, Math.min(frequencies.length, end));
  let total = 0;

  for (let i = from; i < to; i += 1) {
    total += frequencies[i] ?? 0;
  }

  return clamp(total / ((to - from) * 255));
}

export function FerrofluidOrb({ volume, frequencies }: FerrofluidOrbProps) {
  const low = averageBand(frequencies, 0, 8);
  const mids = averageBand(frequencies, 8, 20);
  const highs = averageBand(frequencies, 20, 32);
  const energy = clamp(volume * 0.78 + low * 0.16 + mids * 0.18 + highs * 0.12);

  const energyValue = useMotionValue(0);
  const lowValue = useMotionValue(0);
  const midValue = useMotionValue(0);
  const highValue = useMotionValue(0);

  const liquid = useSpring(energyValue, { stiffness: 80, damping: 14, mass: 0.8 });
  const bass = useSpring(lowValue, { stiffness: 60, damping: 12, mass: 1.0 });
  const voice = useSpring(midValue, { stiffness: 90, damping: 14, mass: 0.7 });
  const texture = useSpring(highValue, { stiffness: 120, damping: 16, mass: 0.5 });

  // 16 motion values for multi-directional satellite offsets
  const topXVal = useMotionValue(0);
  const topYVal = useMotionValue(0);
  const bottomXVal = useMotionValue(0);
  const bottomYVal = useMotionValue(0);
  const leftXVal = useMotionValue(0);
  const leftYVal = useMotionValue(0);
  const rightXVal = useMotionValue(0);
  const rightYVal = useMotionValue(0);

  const tlXVal = useMotionValue(0);
  const tlYVal = useMotionValue(0);
  const trXVal = useMotionValue(0);
  const trYVal = useMotionValue(0);
  const blXVal = useMotionValue(0);
  const blYVal = useMotionValue(0);
  const brXVal = useMotionValue(0);
  const brYVal = useMotionValue(0);

  const energyRef = useRef(0);
  const phaseRef = useRef(0);

  useEffect(() => {
    energyRef.current = energy;
    energyValue.set(energy);
    lowValue.set(low);
    midValue.set(mids);
    highValue.set(highs);
  }, [energy, low, mids, highs, energyValue, lowValue, midValue, highValue]);

  // requestAnimationFrame tick loop to generate chaotic wiggles and multi-directional flows
  useEffect(() => {
    let id: number;
    const tick = () => {
      const currentEnergy = energyRef.current;
      // Increment phase - moves faster when there is higher sound energy
      phaseRef.current += 0.045 + currentEnergy * 0.14;
      const p = phaseRef.current;

      const curBass = bass.get();
      const curVoice = voice.get();
      const curTexture = texture.get();

      // Top satellite (Base cx=50, cy=28) - Radial stretch upwards, lateral wiggle
      const topStretch = curTexture * 26;
      topXVal.set(Math.sin(p * 1.3) * (2 + curTexture * 8));
      topYVal.set(-topStretch + Math.cos(p * 0.9) * (1.5 + curTexture * 4));

      // Bottom satellite (Base cx=50, cy=72) - Radial stretch downwards, lateral wiggle
      const bottomStretch = curBass * 26;
      bottomXVal.set(Math.sin(p * 1.1 + 1) * (2 + curBass * 8));
      bottomYVal.set(bottomStretch + Math.cos(p * 1.4) * (1.5 + curBass * 4));

      // Left satellite (Base cx=28, cy=51) - Radial stretch leftwards, lateral wiggle
      const leftStretch = curVoice * 24;
      leftXVal.set(-leftStretch + Math.sin(p * 1.2) * (1.5 + curVoice * 4));
      leftYVal.set(Math.cos(p * 1.5 + 2) * (2 + curVoice * 8));

      // Right satellite (Base cx=72, cy=51) - Radial stretch rightwards, lateral wiggle
      const rightStretch = curTexture * 24;
      rightXVal.set(rightStretch + Math.sin(p * 1.4 + 3) * (1.5 + curTexture * 4));
      rightYVal.set(Math.cos(p * 1.1) * (2 + curTexture * 8));

      // Diagonal TL (Base cx=34, cy=35) - moves top-left, with circular orbital wobble
      const tlStretch = curVoice * 20;
      tlXVal.set(-tlStretch * 0.707 + Math.sin(p * 1.6 + 4) * (1.5 + curVoice * 5));
      tlYVal.set(-tlStretch * 0.707 + Math.cos(p * 1.2 + 1) * (1.5 + curVoice * 5));

      // Diagonal TR (Base cx=66, cy=35) - moves top-right, with orbital wobble
      const trStretch = curTexture * 20;
      trXVal.set(trStretch * 0.707 + Math.sin(p * 1.5) * (1.5 + curTexture * 5));
      trYVal.set(-trStretch * 0.707 + Math.cos(p * 1.7 + 2) * (1.5 + curTexture * 5));

      // Diagonal BL (Base cx=34, cy=67) - moves bottom-left, with orbital wobble
      const blStretch = curBass * 20;
      blXVal.set(-blStretch * 0.707 + Math.sin(p * 1.3 + 3) * (1.5 + curBass * 5));
      blYVal.set(blStretch * 0.707 + Math.cos(p * 1.4) * (1.5 + curBass * 5));

      // Diagonal BR (Base cx=66, cy=67) - moves bottom-right, with orbital wobble
      const brStretch = curBass * 20;
      brXVal.set(brStretch * 0.707 + Math.sin(p * 1.7) * (1.5 + curBass * 5));
      brYVal.set(brStretch * 0.707 + Math.cos(p * 1.3 + 5) * (1.5 + curBass * 5));

      id = requestAnimationFrame(tick);
    };
    id = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(id);
  }, [bass, voice, texture]);

  const coreScale = useTransform(liquid, [0, 1], [0.94, 1.10]);
  const coreY = useTransform(bass, [0, 1], [2, -6]);
  const satelliteScale = useTransform(liquid, [0, 1], [0.80, 1.25]);
  const smallDropScale = useTransform(texture, [0, 1], [0.50, 1.15]);
  const glowOpacity = useTransform(liquid, [0, 1], [0.18, 0.48]);
  const sheenOpacity = useTransform(liquid, [0, 1], [0.34, 0.72]);

  return (
    <span className="voice-orb" aria-hidden="true">
      <svg className="voice-orb-svg" viewBox="0 0 100 100" role="img">
        <defs>
          <filter id="voice-orb-goo" x="-35%" y="-35%" width="170%" height="170%">
            <feGaussianBlur in="SourceGraphic" stdDeviation="5.5" result="blur" />
            <feColorMatrix
              in="blur"
              mode="matrix"
              values="
                1 0 0 0 0
                0 1 0 0 0
                0 0 1 0 0
                0 0 0 22 -8"
              result="goo"
            />
            <feComposite in="SourceGraphic" in2="goo" operator="atop" />
          </filter>
          <radialGradient id="voice-orb-ink" cx="35%" cy="26%" r="78%">
            <stop offset="0%" stopColor="#2b3442" />
            <stop offset="42%" stopColor="#070b12" />
            <stop offset="100%" stopColor="#000000" />
          </radialGradient>
          <radialGradient id="voice-orb-sheen" cx="30%" cy="22%" r="42%">
            <stop offset="0%" stopColor="#ffffff" stopOpacity="0.72" />
            <stop offset="34%" stopColor="#ffffff" stopOpacity="0.18" />
            <stop offset="100%" stopColor="#ffffff" stopOpacity="0" />
          </radialGradient>
        </defs>

        <motion.circle
          className="voice-orb-soft-glow"
          cx="50"
          cy="50"
          r="43"
          fill="rgba(15,23,42,0.16)"
          style={{ opacity: glowOpacity }}
        />

        <g>
          <g filter="url(#voice-orb-goo)">
            <motion.circle
              className="voice-orb-blob"
              cx="50"
              cy="51"
              r="25"
              fill="url(#voice-orb-ink)"
              style={{ scale: coreScale, y: coreY, transformBox: "fill-box", transformOrigin: "center" }}
              animate={{ x: [-1.2, 1.2, -1.2] }}
              transition={{ duration: 4.1, repeat: Infinity, ease: "easeInOut" }}
            />

            <motion.circle
              className="voice-orb-blob"
              cx="50"
              cy="28"
              r="11"
              fill="url(#voice-orb-ink)"
              style={{ x: topXVal, y: topYVal, scale: satelliteScale, transformBox: "fill-box", transformOrigin: "center" }}
              animate={{ rotate: [-8, 8, -8] }}
              transition={{ duration: 3.5, repeat: Infinity, ease: "easeInOut" }}
            />

            <motion.circle
              className="voice-orb-blob"
              cx="50"
              cy="72"
              r="12"
              fill="url(#voice-orb-ink)"
              style={{ x: bottomXVal, y: bottomYVal, scale: satelliteScale, transformBox: "fill-box", transformOrigin: "center" }}
              animate={{ rotate: [8, -8, 8] }}
              transition={{ duration: 3.8, repeat: Infinity, ease: "easeInOut" }}
            />

            <motion.circle
              className="voice-orb-blob"
              cx="28"
              cy="51"
              r="14"
              fill="url(#voice-orb-ink)"
              style={{ x: leftXVal, y: leftYVal, scale: satelliteScale, transformBox: "fill-box", transformOrigin: "center" }}
              animate={{ rotate: [-6, 6, -6] }}
              transition={{ duration: 3.2, repeat: Infinity, ease: "easeInOut" }}
            />

            <motion.circle
              className="voice-orb-blob"
              cx="72"
              cy="51"
              r="13"
              fill="url(#voice-orb-ink)"
              style={{ x: rightXVal, y: rightYVal, scale: satelliteScale, transformBox: "fill-box", transformOrigin: "center" }}
              animate={{ rotate: [6, -6, 6] }}
              transition={{ duration: 3.4, repeat: Infinity, ease: "easeInOut" }}
            />

            <motion.circle
              className="voice-orb-blob"
              cx="34"
              cy="35"
              r="8"
              fill="url(#voice-orb-ink)"
              style={{ x: tlXVal, y: tlYVal, scale: smallDropScale, transformBox: "fill-box", transformOrigin: "center" }}
              animate={{ rotate: [-6, 6, -6] }}
              transition={{ duration: 2.5, repeat: Infinity, ease: "easeInOut" }}
            />

            <motion.circle
              className="voice-orb-blob"
              cx="66"
              cy="35"
              r="7"
              fill="url(#voice-orb-ink)"
              style={{ x: trXVal, y: trYVal, scale: smallDropScale, transformBox: "fill-box", transformOrigin: "center" }}
              animate={{ rotate: [6, -6, 6] }}
              transition={{ duration: 2.3, repeat: Infinity, ease: "easeInOut" }}
            />

            <motion.circle
              className="voice-orb-blob"
              cx="34"
              cy="67"
              r="9"
              fill="url(#voice-orb-ink)"
              style={{ x: blXVal, y: blYVal, scale: smallDropScale, transformBox: "fill-box", transformOrigin: "center" }}
              animate={{ rotate: [5, -5, 5] }}
              transition={{ duration: 3.2, repeat: Infinity, ease: "easeInOut" }}
            />

            <motion.circle
              className="voice-orb-blob"
              cx="66"
              cy="67"
              r="8"
              fill="url(#voice-orb-ink)"
              style={{ x: brXVal, y: brYVal, scale: smallDropScale, transformBox: "fill-box", transformOrigin: "center" }}
              animate={{ rotate: [-5, 5, -5] }}
              transition={{ duration: 2.9, repeat: Infinity, ease: "easeInOut" }}
            />
          </g>

          <motion.ellipse
            className="voice-orb-sheen"
            cx="39"
            cy="33"
            rx="13"
            ry="7"
            fill="url(#voice-orb-sheen)"
            style={{ opacity: sheenOpacity }}
            animate={{ x: [-1.5, 2, -1.5], y: [0.8, -1.5, 0.8], rotate: [-12, -4, -12] }}
            transition={{ duration: 3.1, repeat: Infinity, ease: "easeInOut" }}
          />
        </g>
      </svg>
    </span>
  );
}
