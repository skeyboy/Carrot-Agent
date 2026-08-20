import readline from "node:readline";
import process from "node:process";

const input = readline.createInterface({ input: process.stdin, crlfDelay: Infinity });
let listCount = 0;

function send(id, result) {
  process.stdout.write(`${JSON.stringify({ jsonrpc: "2.0", id, result })}\n`);
}

function tool(name) {
  return {
    name,
    description: `Dynamic tool ${name}.`,
    inputSchema: { type: "object", additionalProperties: false },
    annotations: { readOnlyHint: true },
  };
}

input.on("line", (line) => {
  const request = JSON.parse(line);
  if (request.id === undefined) return;
  switch (request.method) {
    case "initialize":
      send(request.id, {
        protocolVersion: request.params.protocolVersion,
        capabilities: { tools: { listChanged: true } },
        serverInfo: { name: "carrot-dynamic-mcp", version: "1.0.0" },
      });
      break;
    case "tools/list":
      listCount += 1;
      send(request.id, {
        tools: listCount === 1 ? [tool("first")] : [tool("first"), tool("second")],
      });
      if (listCount === 1) {
        setTimeout(() => {
          process.stdout.write(
            `${JSON.stringify({ jsonrpc: "2.0", method: "notifications/tools/list_changed" })}\n`,
          );
        }, 25);
      }
      break;
    case "ping":
      send(request.id, {});
      break;
    default:
      process.stdout.write(
        `${JSON.stringify({ jsonrpc: "2.0", id: request.id, error: { code: -32601, message: "not found" } })}\n`,
      );
  }
});
