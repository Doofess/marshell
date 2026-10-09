import type { Meta, StoryObj } from "@storybook/react-vite";
import tokensCss from "../styles/tokens.css?raw";
import accentsCss from "../styles/accents.css?raw";
import { parseTokens } from "./cssTokens";
import { contrast, deltaE, minOnSurfaces } from "./contrast";
import { ThemePair } from "./ThemePair";
import "./specimen.css";

const meta: Meta = { title: "Design/Colours" };
export default meta;

const ACCENTS = ["amber", "blue", "indigo", "violet", "magenta", "cyan", "teal", "slate"] as const;
const tokens = parseTokens(tokensCss, ":root");

function Swatch({ name }: { name: string }) {
  return (
    <div className="swatch">
      <div className="swatch__chip" style={{ background: `var(${name})` }} />
      <span className="swatch__name">{name}</span>
    </div>
  );
}

const groups: Array<[string, string[]]> = [
  ["Surfaces", ["--bg-base", "--bg-raised", "--bg-overlay", "--hover", "--selected", "--hairline"]],
  ["Text", ["--text-1", "--text-2", "--text-3"]],
  ["Status", ["--error", "--ok", "--caution"]],
  ["Accent (toolbar)", ["--accent", "--accent-tint", "--accent-flash"]],
  ["Agent brands (stripes and dots only)", ["--brand-claude", "--brand-codex", "--brand-gemini", "--brand-generic"]],
];

export const Palette: StoryObj = {
  render: () => (
    <ThemePair label="Palette">
      {groups.map(([title, names]) => (
        <div key={title}>
          <h3 style={{ fontSize: "var(--text-13)", fontWeight: 600 }}>{title}</h3>
          <div className="specimen-grid">
            {names.map((n) => (
              <Swatch key={n} name={n} />
            ))}
          </div>
        </div>
      ))}
    </ThemePair>
  ),
};

function Ratio({ value, floor }: { value: number; floor: number }) {
  return <span className={value < floor ? "matrix__fail" : undefined}>{value.toFixed(2)}</span>;
}

/** Computed live from the CSS files, so this table can never drift from the tokens. */
export const AccentMatrix: StoryObj = {
  render: () => (
    <table className="matrix">
      <thead>
        <tr>
          <th>Accent</th>
          <th>Theme</th>
          <th>Chip</th>
          <th>On surfaces (≥ 3)</th>
          <th>Ink on accent (≥ 4.5)</th>
          <th>ΔE error / ok / caution (≥ 15)</th>
          <th>ΔE Claude</th>
        </tr>
      </thead>
      <tbody>
        {ACCENTS.flatMap((a) => {
          const v = parseTokens(accentsCss, `[data-accent="${a}"]`);
          return (["dark", "light"] as const).map((theme) => {
            const acc = v["--accent"]![theme];
            return (
              <tr key={a + theme}>
                <td>{a}</td>
                <td>{theme}</td>
                <td>
                  <span
                    style={{
                      display: "inline-block",
                      inlineSize: 40,
                      blockSize: 16,
                      borderRadius: 3,
                      background: acc,
                      color: v["--accent-ink"]![theme],
                      fontSize: 10,
                      textAlign: "center",
                    }}
                  >
                    2
                  </span>
                </td>
                <td>
                  <Ratio value={minOnSurfaces(acc, tokens, theme)} floor={3} />
                </td>
                <td>
                  <Ratio value={contrast(v["--accent-ink"]![theme], acc)} floor={4.5} />
                </td>
                <td>
                  {(["--error", "--ok", "--caution"] as const).map((s, i) => (
                    <span key={s}>
                      {i > 0 && " / "}
                      <Ratio value={deltaE(acc, tokens[s]![theme])} floor={15} />
                    </span>
                  ))}
                </td>
                <td>{deltaE(acc, tokens["--brand-claude"]![theme]).toFixed(1)}</td>
              </tr>
            );
          });
        })}
      </tbody>
    </table>
  ),
};

export const TextMatrix: StoryObj = {
  render: () => (
    <table className="matrix">
      <thead>
        <tr>
          <th>Token</th>
          <th>Theme</th>
          <th>Lowest on base / raised / overlay</th>
          <th>Floor</th>
        </tr>
      </thead>
      <tbody>
        {(["--text-1", "--text-2", "--text-3", "--error", "--ok", "--caution"] as const).flatMap((t) =>
          (["dark", "light"] as const).map((theme) => {
            const floor = t.startsWith("--text") ? 4.5 : 3;
            return (
              <tr key={t + theme}>
                <td>{t}</td>
                <td>{theme}</td>
                <td>
                  <Ratio value={minOnSurfaces(tokens[t]![theme], tokens, theme)} floor={floor} />
                </td>
                <td>{floor}</td>
              </tr>
            );
          }),
        )}
      </tbody>
    </table>
  ),
};
