export function formatDuration(seconds: number) {
  if (!Number.isFinite(seconds)) return "0.0s";
  return `${seconds.toFixed(1)}s`;
}

export function formatDate(value: string) {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return new Intl.DateTimeFormat(undefined, {
    dateStyle: "medium",
    timeStyle: "short",
  }).format(date);
}

export function languageLabel(language: string) {
  if (language === "ru") return "Russian";
  if (language === "en") return "English";
  if (language === "auto") return "Auto";
  return "Unknown";
}
