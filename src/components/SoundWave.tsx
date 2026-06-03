import { useEffect } from "react";
import { motion, useMotionValue, useSpring, useTransform } from "motion/react";
import type { AppStatus } from "../lib/types";

interface SoundWaveProps {
  status: AppStatus;
  audioData: Uint8Array | null;
}

interface WaveBarProps {
  active: boolean;
  processing: boolean;
  index: number;
  level: number;
}

const BAR_COUNT = 18;

function clamp(value: number) {
  return Math.max(0, Math.min(1, value));
}

function readLevel(audioData: Uint8Array | null, index: number) {
  if (!audioData || audioData.length === 0) return 0;

  // Symmetrical mapping: center of the 18 bars (index 8 & 9) represents low frequencies (active voice),
  // tapering off symmetrically to both the left and right edges for higher frequencies.
  const centerIndex = 8.5;
  const dist = Math.abs(index - centerIndex);
  
  // Map distance (0.5 to 8.5) to frequency bin (0 to ~13)
  const bin = Math.min(audioData.length - 1, Math.max(0, Math.round(dist * 1.5)));
  const neighbor = Math.min(audioData.length - 1, bin + 1);
  return clamp(((audioData[bin] ?? 0) * 0.72 + (audioData[neighbor] ?? 0) * 0.28) / 190);
}

function WaveBar({ active, processing, index, level }: WaveBarProps) {
  const levelValue = useMotionValue(0);
  const spring = useSpring(levelValue, {
    stiffness: 360,
    damping: 28,
    mass: 0.36,
  });
  const height = useTransform(spring, [0, 1], [4, 23]);
  const y = useTransform(spring, [0, 1], [0, -2.5]);
  const opacity = useTransform(spring, [0, 1], [0.34, 0.96]);
  const dropOpacity = useTransform(spring, [0.34, 0.88], [0, 0.8]);
  const dropY = useTransform(spring, [0, 1], [2, -8]);
  const dropScale = useTransform(spring, [0, 1], [0.55, 1.12]);

  useEffect(() => {
    levelValue.set(active ? level : 0);
  }, [active, level, levelValue]);

  const idleDelay = index * -0.045;
  const processingDelay = index * -0.055;

  return (
    <span className="sound-wave-cell">
      <motion.span
        className="sound-wave-bar"
        style={active ? { height, y, opacity } : undefined}
        animate={
          processing
            ? {
                height: [7, 18, 8],
                y: [0, -3, 0],
                opacity: [0.36, 0.82, 0.42],
              }
            : active
              ? undefined
              : {
                  height: [5, 11, 5],
                  opacity: [0.24, 0.42, 0.24],
                }
        }
        transition={
          active
            ? undefined
            : {
                duration: processing ? 0.9 : 1.8,
                repeat: Infinity,
                ease: "easeInOut",
                delay: processing ? processingDelay : idleDelay,
              }
        }
      />
      <motion.span
        className="sound-wave-drop"
        style={active ? { opacity: dropOpacity, y: dropY, scale: dropScale } : undefined}
        animate={
          processing
            ? {
                opacity: [0, 0.48, 0],
                y: [2, -7, 2],
                scale: [0.7, 1.05, 0.7],
              }
            : active
              ? undefined
              : { opacity: 0 }
        }
        transition={
          active
            ? undefined
            : {
                duration: 1.05,
                repeat: Infinity,
                ease: "easeInOut",
                delay: processingDelay,
              }
        }
      />
    </span>
  );
}

export function SoundWave({ status, audioData }: SoundWaveProps) {
  const active = status === "recording";
  const processing = status === "processing";

  return (
    <div className="sound-wave-container">
      <svg className="sound-wave-defs" aria-hidden="true" focusable="false">
        <defs>
          <filter id="sound-wave-goo" x="-60%" y="-80%" width="220%" height="260%">
            <feGaussianBlur in="SourceGraphic" stdDeviation="2.2" result="blur" />
            <feColorMatrix
              in="blur"
              mode="matrix"
              values="
                1 0 0 0 0
                0 1 0 0 0
                0 0 1 0 0
                0 0 0 18 -7"
              result="goo"
            />
            <feComposite in="SourceGraphic" in2="goo" operator="atop" />
          </filter>
        </defs>
      </svg>
      <div className={`sound-wave ${active ? "sound-wave-active" : ""} ${processing ? "sound-wave-processing" : ""}`}>
        {Array.from({ length: BAR_COUNT }).map((_, index) => (
          <WaveBar
            key={index}
            active={active}
            processing={processing}
            index={index}
            level={readLevel(audioData, index)}
          />
        ))}
      </div>
    </div>
  );
}
