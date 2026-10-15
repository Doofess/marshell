export type RawResult = { ok: true; value: Record<string, unknown> } | { ok: false; line: number; column: number; message: string };

class JsonError extends Error {
  constructor(
    message: string,
    readonly index: number,
  ) {
    super(message);
  }
}

/**
 * A strict JSON reader that knows where it stopped. JSON.parse does not always say where (V8 sometimes quotes an excerpt
 * instead of a position), and the editor must name the line. Phase 3 swaps this for the core's jsonc CST reader, which also
 * accepts comments; raw mode in phase 0 is strict JSON.
 */
function scan(text: string): void {
  let i = 0;
  const ws = () => {
    while (i < text.length && " \t\r\n".includes(text[i]!)) i++;
  };
  const fail = (m: string): never => {
    throw new JsonError(m, Math.min(i, Math.max(0, text.length - 1)));
  };
  const expect = (c: string) => {
    if (text[i] !== c) fail(`Expected "${c}"${i >= text.length ? " but the file ends" : ` but found "${text[i]}"`}`);
    i++;
  };
  const str = () => {
    expect('"');
    while (i < text.length && text[i] !== '"') {
      if (text[i] === "\n") fail("A string cannot continue on the next line");
      i += text[i] === "\\" ? 2 : 1;
    }
    expect('"');
  };
  const value = (): void => {
    ws();
    const c = text[i];
    if (c === "{") {
      i++;
      ws();
      if (text[i] === "}") return void i++;
      for (;;) {
        ws();
        if (text[i] !== '"') fail(`Expected a property name in double quotes but found ${i >= text.length ? "the end of the file" : `"${text[i]}"`}`);
        str();
        ws();
        expect(":");
        value();
        ws();
        if (text[i] === ",") {
          i++;
          continue;
        }
        if (text[i] === "}") return void i++;
        fail(`Expected "," or "}" after a value but found ${i >= text.length ? "the end of the file" : `"${text[i]}"`}`);
      }
    } else if (c === "[") {
      i++;
      ws();
      if (text[i] === "]") return void i++;
      for (;;) {
        value();
        ws();
        if (text[i] === ",") {
          i++;
          continue;
        }
        if (text[i] === "]") return void i++;
        fail(`Expected "," or "]" after a value but found ${i >= text.length ? "the end of the file" : `"${text[i]}"`}`);
      }
    } else if (c === '"') str();
    else {
      const m = /^(-?\d+(\.\d+)?([eE][+-]?\d+)?|true|false|null)/.exec(text.slice(i));
      if (!m) fail(`Expected a value but found ${i >= text.length ? "the end of the file" : `"${text[i]}"`}`);
      else i += m[0].length;
    }
  };
  value();
  ws();
  if (i < text.length) fail(`Unexpected "${text[i]}" after the end of the object`);
}

function at(text: string, index: number) {
  const before = text.slice(0, index);
  const line = before.split("\n").length;
  return { line, column: index - (before.lastIndexOf("\n") + 1) + 1 };
}

/** Reads the text as one JSON object, or says which line and column is wrong. */
export function parseRaw(text: string): RawResult {
  if (text.trim() === "") return { ok: false, line: 1, column: 1, message: "The file must hold one JSON object." };
  try {
    scan(text);
    const v: unknown = JSON.parse(text);
    if (typeof v !== "object" || v === null || Array.isArray(v)) return { ok: false, line: 1, column: 1, message: "The file must hold one JSON object." };
    return { ok: true, value: v as Record<string, unknown> };
  } catch (e) {
    if (e instanceof JsonError) return { ok: false, ...at(text, e.index), message: e.message };
    return { ok: false, line: 1, column: 1, message: e instanceof Error ? e.message : "The file does not parse." };
  }
}
