import { describe, expect, it } from 'vitest';
import { renderMcpSnippets } from './mcpSettings';

describe('MCP settings snippets', () => {
  it('renders the exact registration and writes-gate syntax', () => {
    expect(renderMcpSnippets('/Applications/Cadence.app/Contents/MacOS/cadence-mcp'))
      .toMatchInlineSnapshot(`
        [
          {
            "id": "claude-code",
            "label": "Claude Code",
            "registration": "claude mcp add cadence --scope user -- /Applications/Cadence.app/Contents/MacOS/cadence-mcp",
            "writesEnabled": "claude mcp add cadence --scope user --env CADENCE_MCP_ALLOW_WRITES=1 -- /Applications/Cadence.app/Contents/MacOS/cadence-mcp",
          },
          {
            "id": "mcp-json",
            "label": ".mcp.json",
            "registration": "{"mcpServers":{"cadence":{"command":"/Applications/Cadence.app/Contents/MacOS/cadence-mcp"}}}",
            "writesEnabled": "{"mcpServers":{"cadence":{"command":"/Applications/Cadence.app/Contents/MacOS/cadence-mcp","env":{"CADENCE_MCP_ALLOW_WRITES":"1"}}}}",
          },
          {
            "id": "codex",
            "label": "Codex",
            "registration": "[mcp_servers.cadence]
        command = "/Applications/Cadence.app/Contents/MacOS/cadence-mcp"
        args = []",
            "writesEnabled": "[mcp_servers.cadence]
        command = "/Applications/Cadence.app/Contents/MacOS/cadence-mcp"
        args = []
        env = { CADENCE_MCP_ALLOW_WRITES = "1" }",
          },
          {
            "id": "gemini",
            "label": "Gemini",
            "registration": "{"mcpServers":{"cadence":{"command":"/Applications/Cadence.app/Contents/MacOS/cadence-mcp"}}}",
            "writesEnabled": "{"mcpServers":{"cadence":{"command":"/Applications/Cadence.app/Contents/MacOS/cadence-mcp","env":{"CADENCE_MCP_ALLOW_WRITES":"1"}}}}",
          },
        ]
      `);
  });

  it('never exposes a database override', () => {
    const serialized = JSON.stringify(renderMcpSnippets('/tmp/cadence-mcp'));
    expect(serialized).not.toContain('CADENCE_DB_PATH');
  });
});

