// The version a release tag gives, and the three files it is stamped into.
export const DRY_RUN_VERSION = "0.1.0-alpha.0";

const TAG = /^v(\d+\.\d+\.\d+-alpha\.[1-9]\d*)$/;

export function versionFromTag(tag) {
  const match = TAG.exec(tag);
  if (!match) {
    throw new Error(`The tag "${tag}" is not a release tag; expected the form v0.1.0-alpha.N.`);
  }
  return match[1];
}

function replaceOnce(text, pattern, replacement, what) {
  const found = text.match(new RegExp(pattern.source, "gm"));
  if (!found || found.length !== 1) {
    throw new Error(`${what} must have exactly one version line, found ${found ? found.length : 0}.`);
  }
  return text.replace(pattern, replacement);
}

function stampJson(text, version, what) {
  return replaceOnce(text, /^(  "version": ")[^"]*(")/m, `$1${version}$2`, what);
}

function stampCargo(text, version) {
  const header = /^\[workspace\.package\][ \t]*\r?$/m.exec(text);
  if (!header) throw new Error("Cargo.toml has no [workspace.package] section.");
  const start = header.index + header[0].length;
  const next = text.slice(start).search(/^\[/m);
  const end = next === -1 ? text.length : start + next;
  const section = replaceOnce(
    text.slice(start, end),
    /^(version = ")[^"]*(")/m,
    `$1${version}$2`,
    "Cargo.toml [workspace.package]",
  );
  return text.slice(0, start) + section + text.slice(end);
}

export function stamp(files, version) {
  return {
    cargoToml: stampCargo(files.cargoToml, version),
    tauriConf: stampJson(files.tauriConf, version, "tauri.conf.json"),
    packageJson: stampJson(files.packageJson, version, "package.json"),
  };
}

export function installerName(version) {
  return `Marshell_${version}_x64-setup.exe`;
}

export function installerUrl(version) {
  return `https://github.com/Doofess/marshell/releases/download/v${version}/${installerName(version)}`;
}
