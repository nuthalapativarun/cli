---
"@googleworkspace/cli": patch
---

fix(services): add apps-script alias for the script service

`helpers/mod.rs` already routes `"apps-script"` to `ScriptHelper`, but the
service registry only listed `"script"` as an alias, so `gws apps-script ...`
returned "Unknown service". Adds `"apps-script"` to the registry so both
aliases resolve identically.
