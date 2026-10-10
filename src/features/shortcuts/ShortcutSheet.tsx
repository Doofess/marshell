import { COMMANDS, NAV_COMMAND_IDS, shortcutLabel, type Command, type Os } from "../../lib/keymap";
import { DEFAULT_NAV_MODIFIER, navModifierLabel, type NavModifier } from "../../lib/navigation/modifier";
import "./ShortcutSheet.css";

type Row = { key: string; label: string; keys: string };
type Group = { title: string; rows: Row[] };

const GROUPS: Array<Command["group"]> = ["Sessions", "View", "Tools"];
const isNav = (c: Command) => (NAV_COMMAND_IDS as readonly string[]).includes(c.id);

/** The cheat sheet's rows, grouped. The nine "Go to session N" keys are one row, and the navigation chords come first. */
export function sheetGroups(os: Os, nav: NavModifier = DEFAULT_NAV_MODIFIER): Group[] {
  const mod = navModifierLabel(os, nav);
  const join = (key: string) => (os === "mac" ? `${mod}${key}` : `${mod}+${key}`);
  const row = (c: Command): Row => ({ key: c.id, label: c.label, keys: shortcutLabel(c.binding!, os, nav) });
  const sessionKeys = COMMANDS.filter((c) => c.id.startsWith("focus-session-"));
  const first = shortcutLabel(sessionKeys[0]!.binding!, os, nav);
  const navigation: Group = {
    title: "Navigation",
    rows: [
      ...COMMANDS.filter(isNav).map(row),
      { key: "nav-arrows", label: "Move the cursor to any terminal, session or control", keys: join("←↑↓→") },
    ],
  };
  const rest = GROUPS.map((title): Group => {
    const rows = COMMANDS.filter((c) => c.group === title && c.binding && !isNav(c) && !c.id.startsWith("focus-session-")).map(row);
    if (title === "Sessions") rows.splice(1, 0, { key: "focus-sessions", label: "Go to session 1 to 9", keys: `${first} to 9` });
    return { title, rows };
  });
  return [navigation, ...rest];
}

/**
 * The keyboard shortcuts for this system. Navigation is explained first because it is the one layer that is not a list of
 * commands: it works on a modifier you hold.
 */
export function ShortcutSheet({ os, nav = DEFAULT_NAV_MODIFIER }: { os: Os; nav?: NavModifier }) {
  const mod = navModifierLabel(os, nav);
  return (
    <section className="shortcuts" aria-labelledby="shortcuts-title">
      <h2 id="shortcuts-title" className="shortcuts__title">
        Keyboard shortcuts
      </h2>
      <p className="shortcuts__intro">
        Hold {mod}, then press Tab or an arrow key. Release to select. Plain Tab, Esc and the arrow keys always go to the program in the terminal.
      </p>
      {sheetGroups(os, nav).map((g) => (
        <div key={g.title} className="shortcuts__group">
          <h3 className="shortcuts__heading">{g.title}</h3>
          <ul className="shortcuts__list">
            {g.rows.map((r) => (
              <li key={r.key} className="shortcuts__row">
                <span className="shortcuts__label">{r.label}</span>
                <kbd className="shortcuts__keys">{r.keys}</kbd>
              </li>
            ))}
          </ul>
        </div>
      ))}
    </section>
  );
}
