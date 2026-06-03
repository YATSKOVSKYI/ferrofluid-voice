interface TranscriptEditorProps {
  value: string;
  onChange: (value: string) => void;
}

export function TranscriptEditor({ value, onChange }: TranscriptEditorProps) {
  return (
    <textarea
      className="transcript-editor"
      value={value}
      onChange={(event) => onChange(event.target.value)}
      placeholder="Your transcript will appear here."
    />
  );
}
