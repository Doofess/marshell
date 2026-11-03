// A fake OpenAI-compatible model for live checks of Qwen Code (slice 6l1 decision 8). Loopback only, Node's
// built-in http, no packages. Usage: node fake-openai.mjs <port> <log.jsonl>
//
// It answers POST /v1/chat/completions (streaming SSE or plain JSON) and GET /v1/models, and appends every
// request body to the log. The reply is picked from the last user message:
//   RUN <command>   a run_shell_command call with that command
//   WRITE <path>    a write_file call (one line of content)
//   EDIT <path>     an edit call replacing "marshell" with "marshell edited"
//   READ <path>     a read_file call
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

/** Fills a tool's arguments from its JSON schema: the value for each property name we know. */
function argsFor(tool, target) {
  const props = tool?.function?.parameters?.properties ?? {};
  const out = {};
  for (const name of Object.keys(props)) {
    if (name === "command") out.command = target;
    else if (name === "is_background") out.is_background = false;
    else if (["file_path", "absolute_path", "path"].includes(name)) out[name] = target;
    else if (name === "content") out.content = "marshell\n";
    else if (name === "old_string") out.old_string = "marshell";
    else if (name === "new_string") out.new_string = "marshell edited";
    else if (name === "description") out.description = "Marshell live check";
  }
  return out;
}

function reply(body) {
  const messages = Array.isArray(body.messages) ? body.messages : [];
  const last = messages[messages.length - 1];
  if (!last || last.role !== "user") return { text: "done" };
  // Qwen sends a user turn as several text parts (system reminders first), so the last command line wins.
  const said = Array.isArray(last.content) ? last.content.map((p) => (typeof p === "string" ? p : p?.text ?? "")).join("\n") : textOf(last.content);
  const verbs = { RUN: "run_shell_command", WRITE: "write_file", EDIT: "edit", READ: "read_file" };
  const matches = [...said.matchAll(/^(RUN|WRITE|EDIT|READ)[ \t]+(.+)$/gm)];
  const m = matches[matches.length - 1];
  if (!m) return { text: "done" };
  const name = verbs[m[1]];
  const tool = (body.tools ?? []).find((t) => t?.function?.name === name);
  if (!tool) return { text: `no ${name} tool offered` };
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
