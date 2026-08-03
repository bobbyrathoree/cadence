export interface McpSnippet {
  id: 'claude-code' | 'mcp-json' | 'codex' | 'gemini';
  label: string;
  registration: string;
  writesEnabled: string;
}

export function renderMcpSnippets(binaryPath: string): McpSnippet[] {
  const jsonBase = JSON.stringify({
    mcpServers: { cadence: { command: binaryPath } },
  });
  const jsonWrites = JSON.stringify({
    mcpServers: {
      cadence: {
        command: binaryPath,
        env: { CADENCE_MCP_ALLOW_WRITES: '1' },
      },
    },
  });

  return [
    {
      id: 'claude-code',
      label: 'Claude Code',
      registration: `claude mcp add cadence --scope user -- ${binaryPath}`,
      writesEnabled:
        `claude mcp add cadence --scope user ` +
        `--env CADENCE_MCP_ALLOW_WRITES=1 -- ${binaryPath}`,
    },
    {
      id: 'mcp-json',
      label: '.mcp.json',
      registration: jsonBase,
      writesEnabled: jsonWrites,
    },
    {
      id: 'codex',
      label: 'Codex',
      registration:
        `[mcp_servers.cadence]\ncommand = "${binaryPath}"\nargs = []`,
      writesEnabled:
        `[mcp_servers.cadence]\ncommand = "${binaryPath}"\nargs = []\n` +
        `env = { CADENCE_MCP_ALLOW_WRITES = "1" }`,
    },
    {
      id: 'gemini',
      label: 'Gemini',
      registration: jsonBase,
      writesEnabled: jsonWrites,
    },
  ];
}

