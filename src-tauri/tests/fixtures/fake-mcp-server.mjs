import readline from "node:readline";
import process from "node:process";

const input = readline.createInterface({ input: process.stdin, crlfDelay: Infinity });

function send(id, result) {
  process.stdout.write(`${JSON.stringify({ jsonrpc: "2.0", id, result })}\n`);
}

input.on("line", (line) => {
  const request = JSON.parse(line);
  if (request.id === undefined) return;
  switch (request.method) {
    case "initialize":
      send(request.id, {
        protocolVersion: request.params.protocolVersion,
        capabilities: { tools: { listChanged: false } },
        serverInfo: { name: "carrot-test-mcp", version: "1.0.0" },
      });
      break;
    case "tools/list":
      send(request.id, {
        tools: [
          {
            name: "read_note",
            title: "Read note",
            description: "Return a deterministic note.",
            inputSchema: {
              type: "object",
              properties: { name: { type: "string" } },
              required: ["name"],
              additionalProperties: false,
            },
            outputSchema: {
              type: "object",
              properties: { note: { type: "string" } },
              required: ["note"],
              additionalProperties: false,
            },
            annotations: { readOnlyHint: true },
          },
        ],
      });
      break;
    case "tools/call": {
      const note = `hello ${request.params.arguments.name}`;
      send(request.id, {
        content: [{ type: "text", text: note }],
        structuredContent: { note },
        isError: false,
      });
      break;
    }
    case "ping":
      send(request.id, {});
      break;
    default:
      process.stdout.write(
        `${JSON.stringify({ jsonrpc: "2.0", id: request.id, error: { code: -32601, message: "not found" } })}\n`,
      );
  }
});
