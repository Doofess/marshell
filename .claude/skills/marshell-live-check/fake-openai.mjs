// A fake OpenAI-compatible model for live checks of Qwen Code (slice 6l1 decision 8) and Copilot CLI (slice 6n1
// decision 8). Loopback only, Node's built-in http, no packages. Usage: node fake-openai.mjs <port> <log.jsonl>
//
// It answers POST /v1/chat/completions (streaming SSE or plain JSON) and GET /v1/models, and appends every
// request body to the log, plus the first request's tool names once. The reply is picked from the last user message:
//   RUN <command>   a shell call with that command (Qwen's run_shell_command, Copilot's powershell or bash)
//   WRITE <path>    a new-file call with one line of content (write_file, create)
//   EDIT <path>     an edit replacing "marshell" with "marshell edited" (edit, str_replace_editor)
//   READ <path>     a read call (read_file, view)
// After a tool result, or for any other text, it replies with the text "done".
// Argument names are read from the tool's schema in the request, so a renamed parameter still gets a value.

import http from "node:http";
import fs from "node:fs";

const port = Number(process.argv[2]);
const log = process.argv[3];
if (!Number.isInteger(port) || !log) {
  console.error("usage: node fake-openai.mjs <port> <log.jsonl>");
  process.exit(2);
}

const MODEL = "marshell-fake";
let callSeq = 0;

function textOf(content) {
  if (typeof content === "string") return content;
  if (Array.isArray(content)) return content.map((p) => (typeof p === "string" ? p : p?.text ?? "")).join("");
  return "";
}

/** Fills a tool's arguments from its JSON schema: the value for each property name we know, and a plain value of the
 *  schema's type for any other required one. */
function argsFor(tool, target) {
  const params = tool?.function?.parameters ?? {};
  const props = params.properties ?? {};
  const required = new Set(params.required ?? []);
  const out = {};
  for (const [name, schema] of Object.entries(props)) {
    if (name === "command") out.command = target;
    else if (name === "is_background") out.is_background = false;
    else if (["file_path", "absolute_path", "path"].includes(name)) out[name] = target;
    else if (["content", "file_text"].includes(name)) out[name] = "marshell\n";
    else if (["old_string", "old_str"].includes(name)) out[name] = "marshell";
    else if (["new_string", "new_str"].includes(name)) out[name] = "marshell edited";
    else if (name === "description") out.description = "Marshell live check";
    else if (required.has(name)) out[name] = plainValue(schema);
  }
  return out;
}

function plainValue(schema) {
  if (Array.isArray(schema?.enum) && schema.enum.length) return schema.enum[0];
  if (schema?.default !== undefined) return schema.default;
  switch (schema?.type) {
    case "number":
    case "integer":
      return 10;
    case "boolean":
      return false;
    case "array":
      return [];
    case "object":
      return {};
    default:
      return "marshell";
  }
}

// The tool each verb calls: the first of these names the request offers (Qwen Code's first, then Copilot CLI's).
const VERB_TOOLS = {
  RUN: ["run_shell_command", "powershell", "bash", "shell"],
  WRITE: ["write_file", "create"],
  EDIT: ["edit", "str_replace_editor", "str_replace"],
  READ: ["read_file", "view"],
};
let toolsLogged = false;

function reply(body) {
  const messages = Array.isArray(body.messages) ? body.messages : [];
  const last = messages[messages.length - 1];
  if (!last || last.role !== "user") return { text: "done" };
  // Qwen sends a user turn as several text parts (system reminders first), so the last command line wins.
  // Copilot's plan mode puts "[[PLAN]] " before the prompt.
  const said = Array.isArray(last.content) ? last.content.map((p) => (typeof p === "string" ? p : p?.text ?? "")).join("\n") : textOf(last.content);
  const matches = [...said.matchAll(/^(?:\[\[[A-Z]+\]\] )?(RUN|WRITE|EDIT|READ)[ \t]+(.+)$/gm)];
  const m = matches[matches.length - 1];
  if (!m) return { text: "done" };
  const offered = body.tools ?? [];
  const names = VERB_TOOLS[m[1]];
  const tool = names.map((n) => offered.find((t) => t?.function?.name === n)).find(Boolean);
  if (!tool) return { text: `no ${names.join(" or ")} tool offered` };
  const name = tool.function.name;
  return { call: { id: `call_${++callSeq}`, name, args: JSON.stringify(argsFor(tool, m[2].trim())) } };
}

function completion(r) {
  const message = r.call
    ? { role: "assistant", content: null, tool_calls: [{ id: r.call.id, type: "function", function: { name: r.call.name, arguments: r.call.args } }] }
    : { role: "assistant", content: r.text };
  return {
    id: `chatcmpl-${Date.now()}`,
    object: "chat.completion",
    created: Math.floor(Date.now() / 1000),
    model: MODEL,
    choices: [{ index: 0, message, finish_reason: r.call ? "tool_calls" : "stop" }],
    usage: { prompt_tokens: 1000, completion_tokens: 10, total_tokens: 1010 },
  };
}

function stream(res, r) {
  res.writeHead(200, { "content-type": "text/event-stream", "cache-control": "no-cache", connection: "keep-alive" });
  const base = { id: `chatcmpl-${Date.now()}`, object: "chat.completion.chunk", created: Math.floor(Date.now() / 1000), model: MODEL };
  const send = (choice, extra = {}) => res.write(`data: ${JSON.stringify({ ...base, choices: [choice], ...extra })}\n\n`);
  if (r.call) {
    send({ index: 0, delta: { role: "assistant", tool_calls: [{ index: 0, id: r.call.id, type: "function", function: { name: r.call.name, arguments: r.call.args } }] }, finish_reason: null });
    send({ index: 0, delta: {}, finish_reason: "tool_calls" });
  } else {
    send({ index: 0, delta: { role: "assistant", content: r.text }, finish_reason: null });
    send({ index: 0, delta: {}, finish_reason: "stop" });
  }
  res.write(`data: ${JSON.stringify({ ...base, choices: [], usage: { prompt_tokens: 1000, completion_tokens: 10, total_tokens: 1010 } })}\n\n`);
  res.end("data: [DONE]\n\n");
}

const server = http.createServer((req, res) => {
  let chunks = [];
  req.on("data", (c) => chunks.push(c));
  req.on("end", () => {
    const raw = Buffer.concat(chunks).toString("utf8");
    fs.appendFileSync(log, JSON.stringify({ at: new Date().toISOString(), method: req.method, url: req.url, body: raw ? safeJson(raw) : null }) + "\n");
    const parsed = raw ? safeJson(raw) : null;
    if (!toolsLogged && Array.isArray(parsed?.tools)) {
      toolsLogged = true;
      fs.appendFileSync(log, JSON.stringify({ at: new Date().toISOString(), tools: parsed.tools.map((t) => t?.function?.name ?? t?.name) }) + "\n");
    }
    if (req.method === "GET" && req.url.endsWith("/models")) {
      res.writeHead(200, { "content-type": "application/json" });
      return res.end(JSON.stringify({ object: "list", data: [{ id: MODEL, object: "model", owned_by: "marshell" }] }));
    }
    if (req.method === "POST" && req.url.endsWith("/chat/completions")) {
      const body = safeJson(raw) ?? {};
      const r = reply(body);
      if (body.stream) return stream(res, r);
      res.writeHead(200, { "content-type": "application/json" });
      return res.end(JSON.stringify(completion(r)));
    }
    res.writeHead(404, { "content-type": "application/json" });
    res.end(JSON.stringify({ error: { message: "not found" } }));
  });
});

function safeJson(text) {
  try {
    return JSON.parse(text);
  } catch {
    return null;
  }
}

server.listen(port, "127.0.0.1", () => console.log(`fake-openai listening on 127.0.0.1:${port}`));
