---
"@googleworkspace/cli": patch
---

Fix `--format` flag being silently ignored in sheets and docs helpers

`sheets +append`, `sheets +read`, and `docs +write` all hard-coded
`OutputFormat::default()` (JSON) when calling `executor::execute_method`,
meaning `--format table`, `--format yaml`, and `--format csv` had no effect.
The handlers now read the global `--format` flag from `ArgMatches` and pass
it through to the executor, consistent with how `calendar +agenda` and
`gmail +triage` already behave.
