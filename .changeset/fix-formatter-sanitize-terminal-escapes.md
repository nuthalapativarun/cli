---
"@googleworkspace/cli": patch
---

fix(formatter): strip terminal escape sequences from non-JSON output

API responses may contain user-generated content with ANSI escape codes or
other control characters. JSON output is safe because serde escapes them as
\uXXXX, but table/CSV/YAML formats passed strings verbatim, allowing a
malicious API value to inject terminal sequences. Adds strip_control_chars()
which is applied to every string cell in value_to_cell().
