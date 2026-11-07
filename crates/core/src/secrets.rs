//! Commands that print or take secrets: which ones hide their output from the recording, and how a command line
//! that holds a secret is shown.
//!
//! Matching is on the normalized program word (basename, lowercase, no extension; launchers such as `npx` are
//! skipped), on the subcommand words that follow it in order, and on flags anywhere after them where a rule says so.
//! A line is split into commands at `;`, `|`, `&&`, `||`, `&` and line breaks, so `cd app && op read ...` counts.
//! The lists follow the research note "Secret-printing commands"; an `[unverified]` entry ships too, because
//! hiding output is the safe failure.

use std::borrow::Cow;

/// A command line's words from the program on, with a leading `&` and a runner (`npx`, `pnpm dlx`, ...) and its
/// flags skipped, or a known tool's runtime and script (`SCRIPTS`) taken as that tool.
pub(crate) struct Command<'a> {
    /// The program word, for a runner the package it runs, or for a node or python child the tool its script is
    /// (basename, lowercase, no `.exe .cmd .bat .ps1`, no `@version`). None when no word is left.
    pub program: Option<String>,
    /// The words after the program.
    pub args: &'a [String],
}

pub(crate) fn split_command(words: &[String]) -> Command<'_> {
    let mut start = 0;
    while words.get(start).is_some_and(|w| w == "&") {
        start += 1;
    }
    let mut cmd = Command {
        program: None,
        args: &[],
    };
    let Some(first) = words.get(start) else {
        return cmd;
    };
    if let Some((program, at)) = by_script(&words[start..]) {
        cmd.program = Some(program.to_string());
        cmd.args = &words[start + at + 1..];
        return cmd;
    }
    let mut at = start;
    if let Some(used) = runner_words(&normalize_word(first), words.get(start + 1).map(|w| normalize_word(w))) {
        at = start + used;
        while words.get(at).is_some_and(|w| w.starts_with('-')) {
            at += 1;
        }
    }
    if let Some(program) = words.get(at) {
        cmd.program = Some(package_name(&normalize_word(program)).to_string());
        cmd.args = &words[at + 1..];
    }
    cmd
}

/// The tools whose Windows shim runs a script: a path part of the script, lowercase with forward slashes, and the
/// program the rules name. `az.cmd` runs `python -IBm azure.cli`, `gcloud.cmd` runs `python ...\lib\gcloud.py`, and
/// an npm-installed CLI runs `node ...\node_modules\<package>\...`. Command Prompt is read from its processes alone,
/// so without these the rules never saw the tool.
const SCRIPTS: &[(&str, &str)] = &[
    ("node_modules/supabase/", "supabase"),
    ("node_modules/vercel/", "vercel"),
    ("node_modules/wrangler/", "wrangler"),
    ("node_modules/netlify-cli/", "netlify"),
    ("node_modules/firebase-tools/", "firebase"),
    ("google-cloud-sdk/lib/gcloud.py", "gcloud"),
    ("azure.cli", "az"),
    ("azure-cli", "az"),
];

/// For a node or python process whose first non-flag word is a known tool's script: that tool's program word and
/// the script word's index, so the words after it are the tool's own.
fn by_script(words: &[String]) -> Option<(&'static str, usize)> {
    let runtime = normalize_word(words.first()?);
    if !(matches!(runtime.as_str(), "node" | "nodejs" | "py") || runtime.starts_with("python")) {
        return None;
    }
    let at = 1 + words[1..].iter().position(|w| !w.starts_with('-'))?;
    let script = collapse_dots(&words[at].replace('\\', "/").to_lowercase());
    SCRIPTS
        .iter()
        .find(|(part, _)| script.contains(part))
        .map(|(_, program)| (*program, at))
}

/// A path with its `.` and `name/..` segments removed: `gcloud.cmd` runs `bin\..\lib\gcloud.py`.
fn collapse_dots(path: &str) -> String {
    let mut out: Vec<&str> = Vec::new();
    for segment in path.split('/') {
        match segment {
            "." => {}
            ".." if out.last().is_some_and(|s| !s.is_empty() && *s != "..") => {
                out.pop();
            }
            _ => out.push(segment),
        }
    }
    out.join("/")
}

/// How many words a runner form takes (`npx` is 1, `pnpm dlx` is 2), or None when the first word is no runner.
fn runner_words(first: &str, second: Option<String>) -> Option<usize> {
    match (first, second.as_deref()) {
        ("npx" | "bunx", _) => Some(1),
        ("pnpm", Some("dlx" | "exec")) | ("npm", Some("exec")) | ("yarn", Some("dlx")) => Some(2),
        _ => None,
    }
}

/// Splits a command line into words, honouring single and double quotes (a backslash is not an escape).
pub(crate) fn tokenize(line: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut cur = String::new();
    let mut quote: Option<char> = None;
    let mut in_word = false;
    for c in line.chars() {
        match quote {
            Some(q) if c == q => quote = None,
            Some(_) => cur.push(c),
            None if c == '"' || c == '\'' => {
                quote = Some(c);
                in_word = true;
            }
            None if c.is_whitespace() => {
                if in_word {
                    words.push(std::mem::take(&mut cur));
                    in_word = false;
                }
            }
            None => {
                cur.push(c);
                in_word = true;
            }
        }
    }
    if in_word {
        words.push(cur);
    }
    words
}

/// A program word as rules name it: no leading `&`, no quotes, the basename, lowercase, without one of
/// `.exe .cmd .bat .ps1`.
fn normalize_word(word: &str) -> String {
    let w = word.trim().trim_start_matches('&').trim();
    let w = w.trim_matches(|c| c == '"' || c == '\'');
    let base = w.rsplit(['\\', '/']).next().unwrap_or(w).to_lowercase();
    for ext in [".exe", ".cmd", ".bat", ".ps1"] {
        if let Some(stem) = base.strip_suffix(ext) {
            return stem.to_string();
        }
    }
    base
}

/// `vercel@latest` is `vercel`. A leading `@` (a scope) stays.
fn package_name(word: &str) -> &str {
    match word.char_indices().skip(1).find(|(_, c)| *c == '@') {
        Some((i, _)) => &word[..i],
        None => word,
    }
}

/// How many positional words (non-flags) after the program a rule's subcommand words are looked for in.
const WORD_WINDOW: usize = 8;

/// Flags that put a secret on the command line for any program, with a space or `=` before the value.
const SECRET_FLAGS: &[&str] = &[
    "--token",
    "--access-token",
    "--api-key",
    "--auth",
    "--password",
    "--secret",
    "--from-literal",
    "--basic-auth",
];

/// What else a rule needs, beyond the program and its subcommand words.
enum Cond {
    Always,
    /// One of these flags is present (`--flag`, `--flag=v`, and for a short flag `-fv`).
    Flag(&'static [&'static str]),
    /// One of these words is among the arguments.
    Arg(&'static [&'static str]),
    /// An argument contains one of these (lowercase) texts.
    Text(&'static [&'static str]),
    /// The `-o` or `--output` value starts with one of these.
    Output(&'static [&'static str]),
    /// A positional argument is a `key=value` assignment.
    Assign,
}

struct Rule {
    program: &'static str,
    words: &'static [&'static str],
    cond: Cond,
    /// The rule does not apply when one of these flags is present.
    without: &'static [&'static str],
}

const fn rule(program: &'static str, words: &'static [&'static str], cond: Cond) -> Rule {
    Rule {
        program,
        words,
        cond,
        without: &[],
    }
}

use Cond::{Always, Arg, Assign, Flag, Output, Text};

const STRUCTURED: &[&str] = &["yaml", "json", "jsonpath", "go-template"];

/// Commands whose output holds secrets.
static PRINTS: &[Rule] = &[
    rule("op", &["read"], Always),
    rule(
        "op",
        &["item", "get"],
        Flag(&["--reveal", "--fields", "--otp", "--share-link", "--format"]),
    ),
    rule("op", &["document", "get"], Always),
    Rule {
        without: &["-o", "--out-file"],
        ..rule("op", &["inject"], Always)
    },
    rule("op", &["run"], Flag(&["--no-masking"])),
    rule("op", &["signin"], Always),
    rule("op", &["account", "add"], Flag(&["--raw"])),
    rule("op", &["service-account", "create"], Always),
    rule("op", &["connect", "token", "create"], Always),
    rule("docker", &["compose", "config"], Always),
    rule("docker", &["inspect"], Always),
    rule("docker", &["exec"], Arg(&["env", "printenv"])),
    rule("docker", &["compose", "run"], Arg(&["env", "printenv"])),
    rule("docker", &["compose", "exec"], Arg(&["env", "printenv"])),
    rule("docker", &["logs"], Always),
    rule("docker", &["context", "inspect"], Always),
    rule("docker", &["buildx", "build"], Flag(&["--secret", "--build-arg"])),
    rule("supabase", &["start"], Always),
    rule("supabase", &["status"], Always),
    rule("supabase", &["login"], Flag(&["--token"])),
    rule("supabase", &["projects", "api-keys"], Always),
    rule("supabase", &["secrets", "list"], Always),
    rule("supabase", &["db", "dump"], Always),
    rule("gh", &["auth", "token"], Always),
    rule("gh", &["auth", "status"], Flag(&["--show-token", "-t"])),
    rule("gh", &["config", "get"], Always),
    rule("gh", &["api"], Flag(&["--include", "-i"])),
    rule("gh", &["api"], Text(&["authorization"])),
    rule("kubectl", &["get", "secret"], Output(STRUCTURED)),
    rule("kubectl", &["config", "view"], Flag(&["--raw"])),
    rule("kubectl", &["create", "token"], Always),
    rule("kubectl", &["get", "pod"], Output(STRUCTURED)),
    rule("kubectl", &["exec"], Arg(&["env", "printenv"])),
    rule("kubectl", &["logs"], Always),
    rule("vercel", &["env", "pull"], Always),
    rule("vercel", &["env", "run"], Always),
    rule("vercel", &["env", "add"], Always),
    rule("vercel", &["env", "ls"], Always),
    rule("vercel", &["api"], Always),
    rule("wrangler", &["auth", "token"], Always),
    rule("wrangler", &["secret", "put"], Always),
    rule("wrangler", &["secret", "bulk"], Always),
    rule("wrangler", &["whoami"], Always),
    rule("wrangler", &["d1", "execute"], Flag(&["--remote"])),
    rule("wrangler", &["kv", "key", "get"], Always),
    rule("firebase", &["login:ci"], Always),
    rule("firebase", &["functions:secrets:access"], Always),
    rule("firebase", &["apps:sdkconfig"], Always),
    rule("firebase", &["auth:export"], Always),
    rule("aws", &["configure", "export-credentials"], Always),
    rule("aws", &["configure", "get", "aws_secret_access_key"], Always),
    rule("aws", &["configure", "list"], Always),
    rule("aws", &["sts", "assume-role"], Always),
    rule("aws", &["sts", "get-session-token"], Always),
    rule("aws", &["sts", "get-federation-token"], Always),
    rule("aws", &["sso", "get-role-credentials"], Always),
    rule("aws", &["iam", "create-access-key"], Always),
    rule("aws", &["eks", "get-token"], Always),
    rule("aws", &["ecr", "get-login-password"], Always),
    rule("aws", &["secretsmanager", "get-secret-value"], Always),
    rule("aws", &["ssm", "get-parameter"], Flag(&["--with-decryption"])),
    rule("aws", &["codeartifact", "get-authorization-token"], Always),
    rule("aws", &["kms", "decrypt"], Always),
    rule("aws", &["lambda", "get-function-configuration"], Always),
    rule("aws", &["apigateway", "get-api-keys"], Flag(&["--include-values"])),
    rule("aws", &["rds", "generate-db-auth-token"], Always),
    rule("gcloud", &["auth", "print-access-token"], Always),
    rule("gcloud", &["auth", "print-identity-token"], Always),
    rule("gcloud", &["auth", "application-default", "print-access-token"], Always),
    rule("gcloud", &["config", "config-helper"], Always),
    rule("gcloud", &["secrets", "versions", "access"], Always),
    rule("gcloud", &["iam", "service-accounts", "keys", "create"], Always),
    rule("gcloud", &["kms", "decrypt"], Always),
    rule("gcloud", &["sql", "users", "set-password"], Always),
    rule("gcloud", &["compute", "instances", "get-serial-port-output"], Always),
    rule("gcloud", &["run", "services", "describe"], Always),
    rule("gcloud", &["functions", "describe"], Always),
    rule("az", &["account", "get-access-token"], Always),
    rule("az", &["ad", "sp", "create-for-rbac"], Always),
    rule("az", &["keyvault", "secret", "show"], Always),
    rule("az", &["keyvault", "secret", "download"], Always),
    rule("az", &["storage", "account", "keys", "list"], Always),
    rule("az", &["acr", "credential", "show"], Always),
    rule("az", &["webapp", "config", "appsettings", "list"], Always),
    rule("az", &["ad", "app", "credential", "reset"], Always),
    rule("az", &["storage", "account", "show-connection-string"], Always),
    rule("az", &["cosmosdb", "keys", "list"], Always),
    rule("az", &["servicebus", "authorization-rule", "keys", "list"], Always),
    rule("az", &["functionapp", "config", "appsettings", "list"], Always),
    rule("az", &["acr", "login"], Flag(&["--expose-token"])),
    rule("az", &["sql", "show-connection-string"], Always),
    rule("terraform", &["output"], Flag(&["-json", "-raw"])),
    rule("terraform", &["show"], Flag(&["-json"])),
    rule("terraform", &["state", "show"], Always),
    rule("terraform", &["state", "pull"], Always),
    rule("terraform", &["login"], Always),
    rule("fly", &["auth", "token"], Always),
    rule("fly", &["tokens", "create"], Always),
    rule("fly", &["ssh", "issue"], Always),
    rule("fly", &["redis", "proxy"], Always),
    rule("fly", &["postgres", "connect"], Always),
    rule("fly", &["mpg"], Always),
    rule("netlify", &["env:list"], Always),
    rule("netlify", &["env:get"], Always),
    rule("netlify", &["env:export"], Always),
    rule("netlify", &["api"], Always),
    rule("stripe", &["listen"], Always),
    rule("stripe", &["config"], Flag(&["--list"])),
    rule("stripe", &["login"], Flag(&["--interactive"])),
    rule("stripe", &["get"], Always),
];

/// Commands that take a secret on the command line, besides `SECRET_FLAGS`.
static TAKES: &[Rule] = &[
    rule("docker", &["login"], Flag(&["-p"])),
    rule("az", &["login"], Flag(&["-p"])),
    rule("fly", &[], Flag(&["-t"])),
    rule("terraform", &[], Flag(&["-var"])),
    rule("gh", &["secret", "set"], Flag(&["-b", "--body"])),
    rule(
        "kubectl",
        &["create", "secret", "docker-registry"],
        Flag(&["--docker-password"]),
    ),
    rule("ngrok", &["config", "add-authtoken"], Always),
    rule("fly", &["secrets", "set"], Always),
    rule("aws", &["configure", "set"], Always),
    rule("op", &["item", "create"], Assign),
    rule("op", &["item", "edit"], Assign),
];

/// Whether the command line runs a command that prints secrets.
pub fn prints_secrets(line: &str) -> bool {
    commands(line).iter().any(|c| prints_secrets_words(c))
}

/// `line` as it may be shown. A line holding a secret flag or argument becomes `shown(line)`; any other line is
/// returned as it is.
pub fn scrub(line: &str) -> Cow<'_, str> {
    if commands(line).iter().any(|c| takes_secret_words(c)) {
        Cow::Owned(shown(line))
    } else {
        Cow::Borrowed(line)
    }
}

/// The program word and up to two subcommand words, then ` …`. Of a line with several commands it shows the first
/// one that prints or takes a secret, else the first. Words after a flag are left out: they may be its value.
pub fn shown(line: &str) -> String {
    let all = commands(line);
    let pick = all
        .iter()
        .find(|c| prints_secrets_words(c) || takes_secret_words(c))
        .or_else(|| all.first());
    pick.map_or_else(|| "…".to_string(), |c| shown_words(c))
}

/// `prints_secrets` for one command's words (an argument vector, or a tokenized line without `;`, `|`, ...).
pub(crate) fn prints_secrets_words(words: &[String]) -> bool {
    matches_any(PRINTS, words)
}

/// Whether one command's words hold a secret flag or argument.
pub(crate) fn takes_secret_words(words: &[String]) -> bool {
    let cmd = split_command(words);
    cmd.program.is_some() && (has_flag(cmd.args, SECRET_FLAGS, false) || matches_any(TAKES, words))
}

/// `shown` for one command's words.
pub(crate) fn shown_words(words: &[String]) -> String {
    let cmd = split_command(words);
    let Some(program) = cmd.program else {
        return "…".to_string();
    };
    let mut out = program;
    for w in cmd.args.iter().take_while(|w| !w.starts_with('-')).take(2) {
        out.push(' ');
        out.push_str(w);
    }
    out.push_str(" …");
    out
}

/// A line's commands, each as its words. Line breaks, `;`, `|`, `||`, `&&` and `&` end a command.
fn commands(line: &str) -> Vec<Vec<String>> {
    let mut out = Vec::new();
    for row in line.lines() {
        let mut cur: Vec<String> = Vec::new();
        for word in tokenize(row) {
            if matches!(word.as_str(), ";" | "|" | "||" | "&&" | "&") {
                out.push(std::mem::take(&mut cur));
                continue;
            }
            match word.strip_suffix([';', '|']) {
                Some(stem) if !stem.is_empty() => {
                    cur.push(stem.to_string());
                    out.push(std::mem::take(&mut cur));
                }
                _ => cur.push(word),
            }
        }
        out.push(cur);
    }
    out.retain(|c| !c.is_empty());
    out
}

/// The program word under the names a tool goes by.
fn canonical(program: &str) -> &str {
    match program {
        "flyctl" => "fly",
        "firebase-tools" => "firebase",
        "netlify-cli" => "netlify",
        other => other,
    }
}

fn matches_any(rules: &[Rule], words: &[String]) -> bool {
    let cmd = split_command(words);
    let Some(program) = cmd.program else {
        return false;
    };
    let program = canonical(&program);
    rules.iter().any(|r| r.program == program && applies(r, cmd.args))
}

/// Whether the positional argument `a` is the word `w`, its plural, or either as the type of `type/name`.
fn word_is(a: &str, w: &str) -> bool {
    let kind = a.split('/').next().unwrap_or(a);
    kind == w || kind.strip_suffix('s') == Some(w)
}

fn applies(r: &Rule, args: &[String]) -> bool {
    let positional: Vec<String> = args
        .iter()
        .filter(|a| !a.starts_with('-'))
        .take(WORD_WINDOW)
        .map(|a| a.to_lowercase())
        .collect();
    let mut at = positional.iter();
    let words_found = r.words.iter().all(|w| at.any(|a| word_is(a, w)));
    if !words_found || has_flag(args, r.without, true) {
        return false;
    }
    match &r.cond {
        Always => true,
        Flag(flags) => has_flag(args, flags, true),
        Arg(names) => args.iter().any(|a| names.contains(&a.as_str())),
        Text(texts) => args.iter().any(|a| {
            let a = a.to_lowercase();
            texts.iter().any(|t| a.contains(t))
        }),
        Output(values) => output_value(args).is_some_and(|v| values.iter().any(|x| v.starts_with(x))),
        Assign => args.iter().any(|a| !a.starts_with('-') && a.contains('=')),
    }
}

/// Whether one of `flags` is among `args`: `--flag`, `--flag=value`, and with `short` also `-fvalue` for a
/// one-letter flag.
fn has_flag(args: &[String], flags: &[&str], short: bool) -> bool {
    args.iter().any(|a| {
        flags.iter().any(|f| {
            a == f
                || a.strip_prefix(f).is_some_and(|rest| rest.starts_with('='))
                || (short && f.len() == 2 && !f.starts_with("--") && a.starts_with(f))
        })
    })
}

/// The value of `-o` or `--output`, lowercase.
fn output_value(args: &[String]) -> Option<String> {
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let value = if a == "-o" || a == "--output" {
            it.next().cloned()
        } else if let Some(v) = a.strip_prefix("--output=").or_else(|| a.strip_prefix("-o=")) {
            Some(v.to_string())
        } else if !a.starts_with("--") && a.len() > 2 {
            a.strip_prefix("-o").map(str::to_string)
        } else {
            None
        };
        if let Some(v) = value {
            return Some(v.to_lowercase());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secret_commands_are_recognised() {
        for line in [
            "op read op://a/b/c",
            "npx supabase status -o env",
            "gh auth token",
            "kubectl get secret db -o yaml",
            "kubectl -n prod get secrets db --output=json",
            "kubectl get secret db -ojsonpath={.data}",
            "kubectl get secret/db -o yaml",
            "kubectl get pods/web -o json",
            "kubectl config view --raw",
            "kubectl get pod web -o yaml",
            "kubectl exec web -- printenv",
            "gh auth status --show-token",
            "gh api /user --include",
            "gh api /user -H \"Authorization: bearer x\"",
            "gh config get oauth_token",
            "op item get Login --reveal",
            "op item get Login --fields password",
            "op item get Login --otp",
            "op item get Login --share-link",
            "op item get Login --format json",
            "op document get f",
            "op inject -i a.tpl",
            "op run --no-masking -- env",
            "op signin --raw",
            "op signin",
            "op account add --raw",
            "op service-account create x",
            "op connect token create x",
            "supabase start",
            "pnpm dlx supabase start",
            "npm exec -- supabase status",
            "C:\\tools\\Supabase.exe status",
            "& 'C:\\Program Files\\GitHub CLI\\gh.exe' auth token",
            "cd app && op read op://a/b/c",
            "echo hi; gh auth token",
            "echo hi | gh auth token",
            "docker compose config",
            "docker -H tcp://h compose -f x.yml config",
            "docker inspect web",
            "docker exec web env",
            "docker compose run web printenv",
            "docker logs web",
            "docker context inspect",
            "docker buildx build --secret id=a,src=b .",
            "supabase login --token x",
            "supabase projects api-keys",
            "supabase secrets list",
            "supabase db dump",
            "vercel env pull",
            "vercel env run -- x",
            "vercel env add A",
            "vercel env ls",
            "vercel api /v1",
            "wrangler auth token",
            "wrangler secret put A",
            "wrangler secret bulk f",
            "wrangler whoami",
            "wrangler d1 execute db --remote",
            "wrangler kv key get k",
            "firebase login:ci",
            "firebase functions:secrets:access A",
            "firebase apps:sdkconfig",
            "firebase auth:export f",
            "aws configure export-credentials",
            "aws configure get aws_secret_access_key",
            "aws configure list",
            "aws --profile p sts assume-role --role-arn r",
            "aws sts get-session-token",
            "aws sts get-federation-token",
            "aws sso get-role-credentials",
            "aws iam create-access-key",
            "aws eks get-token --cluster-name c",
            "aws ecr get-login-password",
            "aws secretsmanager get-secret-value --secret-id s",
            "aws ssm get-parameter --name n --with-decryption",
            "aws codeartifact get-authorization-token",
            "aws kms decrypt --key-id k",
            "aws lambda get-function-configuration --function-name f",
            "aws apigateway get-api-keys --include-values",
            "aws rds generate-db-auth-token",
            "gcloud auth print-access-token",
            "gcloud auth print-identity-token",
            "gcloud auth application-default print-access-token",
            "gcloud config config-helper",
            "gcloud secrets versions access latest --secret=s",
            "gcloud iam service-accounts keys create k.json",
            "gcloud kms decrypt --key k",
            "gcloud sql users set-password u --password p",
            "gcloud compute instances get-serial-port-output vm",
            "gcloud run services describe s",
            "gcloud functions describe f",
            "az account get-access-token",
            "az ad sp create-for-rbac",
            "az keyvault secret show --name n",
            "az keyvault secret download --name n",
            "az storage account keys list",
            "az acr credential show",
            "az webapp config appsettings list",
            "az ad app credential reset",
            "az storage account show-connection-string",
            "az cosmosdb keys list",
            "az servicebus namespace authorization-rule keys list",
            "az functionapp config appsettings list",
            "az acr login --expose-token",
            "az sql db show-connection-string",
            "terraform output -json",
            "terraform output -raw x",
            "terraform show -json",
            "terraform state show a.b",
            "terraform state pull",
            "terraform login",
            "fly auth token",
            "fly tokens create",
            "fly ssh issue",
            "fly redis proxy",
            "flyctl postgres connect",
            "fly mpg list",
            "netlify env:list",
            "netlify env:get A",
            "netlify env:export",
            "netlify api listSites",
            "stripe listen",
            "stripe listen --print-secret",
            "stripe config --list",
            "stripe login --interactive",
            "stripe get /v1/customers",
        ] {
            assert!(prints_secrets(line), "{line}");
        }
    }

    #[test]
    fn ordinary_commands_are_not_secret_ones() {
        for line in [
            "op whoami",
            "kubectl get pods",
            "kubectl get pods -o wide",
            "kubectl get secret db -o name",
            "kubectl create secret generic s --from-literal=k=v",
            "gh auth status",
            "gh auth login --with-token",
            "gh pr list",
            "op inject -i a.tpl -o out",
            "op item get Login",
            "docker ps",
            "docker compose up",
            "docker login --password-stdin",
            "supabase db push",
            "vercel deploy",
            "wrangler dev",
            "aws s3 ls",
            "aws configure",
            "gcloud auth login",
            "gcloud auth activate-service-account --key-file k.json",
            "az login",
            "terraform plan",
            "terraform output",
            "fly deploy",
            "stripe trigger payment_intent.created",
            "git status",
            "echo gh auth token",
            "",
        ] {
            assert!(!prints_secrets(line), "{line}");
        }
    }

    #[test]
    fn secret_flags_are_scrubbed() {
        let cases = [
            ("docker login -u me -p hunter2", "docker login …"),
            ("docker login -u me --password hunter2", "docker login …"),
            ("vercel deploy --token=abc", "vercel deploy …"),
            ("vercel deploy --token abc", "vercel deploy …"),
            (
                "kubectl create secret generic s --from-literal=k=v",
                "kubectl create secret …",
            ),
            (
                "kubectl create secret docker-registry r --docker-password=hunter2",
                "kubectl create secret …",
            ),
            ("az login --service-principal -u app -p hunter2", "az login …"),
            ("fly status -t hunter2", "fly status …"),
            ("fly -t hunter2 status", "fly …"),
            ("terraform apply -var db_pass=hunter2", "terraform apply …"),
            ("terraform apply -var=db_pass=hunter2", "terraform apply …"),
            ("gh secret set NAME -b hunter2", "gh secret set …"),
            ("gh secret set NAME --body hunter2", "gh secret set …"),
            ("ngrok config add-authtoken hunter2", "ngrok config add-authtoken …"),
            ("fly secrets set KEY=hunter2", "fly secrets set …"),
            ("aws configure set aws_secret_access_key hunter2", "aws configure set …"),
            ("op item create --title t password=hunter2", "op item create …"),
            ("op item edit Login password=hunter2", "op item edit …"),
            ("stripe listen --api-key hunter2", "stripe listen …"),
            ("netlify deploy --auth hunter2", "netlify deploy …"),
            ("some-tool --api-key=hunter2", "some-tool …"),
            ("some-tool --access-token hunter2", "some-tool …"),
            ("some-tool --secret hunter2", "some-tool …"),
            ("ngrok http 80 --basic-auth user:hunter2", "ngrok http 80 …"),
            ("npx vercel deploy --token=hunter2", "vercel deploy …"),
            ("cd app && docker login -p hunter2", "docker login …"),
            ("& 'C:\\bin\\docker.exe' login -p hunter2", "docker login …"),
        ];
        for (line, want) in cases {
            let got = scrub(line);
            assert_eq!(got, want, "{line}");
            assert!(!got.contains("hunter2"), "{line}");
        }
    }

    #[test]
    fn lines_without_a_secret_are_left_alone() {
        for line in [
            "git status",
            "docker login --password-stdin",
            "docker login -u me",
            "op item get Login",
            "terraform apply -var-file=prod.tfvars",
            "gh secret list",
            "",
        ] {
            assert!(matches!(scrub(line), Cow::Borrowed(l) if l == line), "{line}");
        }
    }

    #[test]
    fn shown_names_the_secret_command_not_the_rest() {
        assert_eq!(shown("gh auth token"), "gh auth token …");
        assert_eq!(shown("echo hunter2 && gh auth token"), "gh auth token …");
        assert_eq!(shown("op read op://a/b/c extra"), "op read op://a/b/c …");
        assert_eq!(shown("git"), "git …");
        assert_eq!(shown("git status --short"), "git status …");
        assert_eq!(shown(""), "…");
    }

    #[test]
    fn a_node_or_python_child_is_named_by_its_script() {
        let argv = |a: &[&str]| a.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        // What `supabase.cmd`, `az.cmd` and `gcloud.cmd` really run: Command Prompt is read by its processes alone.
        for (words, label) in [
            (
                argv(&["node", r"C:\x\node_modules\supabase\bin\main.js", "status"]),
                "supabase status …",
            ),
            (
                argv(&[
                    r"C:\Program Files\Microsoft SDKs\Azure\CLI2\python.exe",
                    "-IBm",
                    "azure.cli",
                    "account",
                    "get-access-token",
                ]),
                "az account get-access-token …",
            ),
            (
                argv(&[
                    "python.exe",
                    "-S",
                    r"C:\Program Files (x86)\Google\Cloud SDK\google-cloud-sdk\bin\..\lib\gcloud.py",
                    "auth",
                    "print-access-token",
                ]),
                "gcloud auth print-access-token …",
            ),
            (
                argv(&[
                    "node.exe",
                    r"C:\Users\a b\AppData\Roaming\npm\node_modules\firebase-tools\lib\bin\firebase.js",
                    "login:ci",
                ]),
                "firebase login:ci …",
            ),
            (
                argv(&["node", "/home/a/.npm/node_modules/netlify-cli/bin/run.js", "env:list"]),
                "netlify env:list …",
            ),
        ] {
            assert!(prints_secrets_words(&words), "{words:?}");
            assert_eq!(shown_words(&words), label, "{words:?}");
        }
        // A script that is no tool, and a runtime with no script, stay the runtime.
        assert!(!prints_secrets_words(&argv(&[
            "node",
            r"C:\x\node_modules\vercel-like\x.js",
            "env",
            "pull"
        ])));
        assert!(!prints_secrets_words(&argv(&["python", "-m", "http.server", "8765"])));
        assert_eq!(shown_words(&argv(&["python", "-m", "http.server"])), "python …");
        // A typed line names it the same way, and a token on it is scrubbed.
        assert!(prints_secrets("node node_modules/supabase/bin/main.js status"));
        assert_eq!(
            scrub("node node_modules/vercel/dist/index.js env pull --token hunter2"),
            "vercel env pull …"
        );
    }
}
