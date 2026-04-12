---
"@googleworkspace/cli": patch
---

docs(drive): add shared drives tip to SKILL.md

Files in shared drives are invisible to `files list` and `files get`
by default. Adds a "Shared Drives" section explaining that agents
should retry with `supportsAllDrives: true` and
`includeItemsFromAllDrives: true` when a file search returns no
results and the user indicates the file is in a shared drive.
