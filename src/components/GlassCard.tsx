import type { PropsWithChildren } from "react";
import { clsx } from "clsx";

interface GlassCardProps extends PropsWithChildren {
  className?: string;
}

export function GlassCard({ children, className }: GlassCardProps) {
  return <div className={clsx("glass-panel", className)}>{children}</div>;
}
