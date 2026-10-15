const graphemes = new Intl.Segmenter(undefined, { granularity: "grapheme" });
const first = (s: string) => [...graphemes.segment(s)][0]?.segment ?? "";

/** Two characters that stand for a session in the 52 px rail: word initials, else the first two characters. */
export function monogram(name: string): string {
  const words = name.split(/[\s\-_./\\]+/).filter(Boolean);
  if (words.length >= 2) return (first(words[0]!) + first(words[1]!)).toLocaleUpperCase();
  const chars = [...graphemes.segment(words[0] ?? "")].map((g) => g.segment);
  return chars.slice(0, 2).join("").toLocaleUpperCase();
}
