/** Name the MCP server goes by in assistants' settings. */
export const SERVER_NAME = "octopuson-platform";

/** What a token looks like before it is made (in examples). */
export const TOKEN_PLACEHOLDER = "opt_…";

/** The web server's MCP address. */
export function mcpUrl(origin: string): string {
  return `${origin.replace(/\/+$/, "")}/mcp`;
}

/** MCP settings that start `command` with `mcp` (Claude Desktop, Cursor, …): the desktop program itself, or `octopuson-platform`. */
export function desktopConfig(command: string = SERVER_NAME): string {
  return JSON.stringify({ mcpServers: { [SERVER_NAME]: { command, args: ["mcp"] } } }, null, 2).replace(/\[\s+"mcp"\s+\]/, '["mcp"]');
}

/** MCP settings for the web server, with `token` in the header. */
export function webConfig(url: string, token: string): string {
  return JSON.stringify({ mcpServers: { [SERVER_NAME]: { type: "http", url, headers: { Authorization: `Bearer ${token}` } } } }, null, 2);
}

/** A program path as one shell word. */
function shellWord(text: string): string {
  return /^[\w@%+=:,./-]+$/.test(text) ? text : `'${text.replace(/'/g, "'\\''")}'`;
}

/** The Claude Code command that adds the server: the web address with `token`, or `command` started with `mcp`. */
export function claudeCodeCommand(url: string | null, token: string, command: string = SERVER_NAME): string {
  return url ? `claude mcp add --transport http ${SERVER_NAME} ${url} --header "Authorization: Bearer ${token}"` : `claude mcp add ${SERVER_NAME} -- ${shellWord(command)} mcp`;
}

/** Unix seconds as an ISO time (for `relativeTime`). */
export function isoOf(seconds: number): string {
  return new Date(seconds * 1000).toISOString();
}
