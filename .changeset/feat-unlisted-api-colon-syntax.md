---
"@googleworkspace/cli": minor
---

feat: implement <api>:<version> syntax for unlisted Discovery APIs

`gws admob:v1 <resource> <method>` now fetches the Discovery Document
directly from the Discovery Service without requiring a registry entry.
Previously the colon syntax only overrode the version for already-registered
services; unlisted API names were rejected regardless.
