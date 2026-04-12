---
"@googleworkspace/cli": patch
---

docs(skills): add content creation guidance to gws-drive SKILL.md

Point agents to the Sheets, Docs, and Slides APIs when creating files with
content. `drive files create` with a Google MIME type produces an empty shell;
the guidance prevents agents from silently losing data.
