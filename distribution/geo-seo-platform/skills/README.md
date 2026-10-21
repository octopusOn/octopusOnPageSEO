# OctopusOn GEO-SEO Platform: agent skills

Five skills that teach an AI assistant to work with the platform's MCP server:

| Skill | What the assistant does |
|---|---|
| `seo-audit` | Turns the site audit, Search Console and rank tracking into a prioritised fix list |
| `keyword-research` | Finds, sizes and groups keywords for a project and maps them to pages |
| `competitor-analysis` | Compares a project with its rivals: traffic, keywords, links, AI answers |
| `geo-visibility` | Measures and improves how AI answers (ChatGPT, Claude, Gemini, Perplexity, AI Overviews) mention the brand |
| `seo-coach` | A weekly check-in that explains what changed and what to do next, in plain words |

Every skill asks the user before buying data: paid tools return the price first, and the assistant calls
them again with `allow_spend_usd` only after a yes.

## Connect the MCP server first

- **Desktop program:** add `{"mcpServers":{"octopuson-platform":{"command":"octopuson-platform","args":["mcp"]}}}`
  to the assistant's MCP settings (Settings → AI assistants in the program shows it).
- **Web (app.octopuson.com):** make an access token under Settings → AI assistants and give the assistant
  `https://app.octopuson.com/mcp` with the header `Authorization: Bearer opt_…`.

## Install the skills

- **Claude Code:** copy the skill folders to `~/.claude/skills/` (or a project's `.claude/skills/`).
- **Claude apps:** zip a skill folder and upload it under Settings → Capabilities → Skills.
- **Other assistants:** paste a skill's `SKILL.md` into the assistant's instructions.
