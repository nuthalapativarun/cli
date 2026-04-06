---
"@googleworkspace/cli": patch
---

Add unit tests for dangerous Unicode and null/control characters on `--dir` and file path validation, matching `--output-dir` coverage. Add a project `.cursor/mcp.json` template for the official GitHub MCP server (replace `YOUR_GITHUB_PAT`) and a `.gitignore` exception so the template can be committed.
