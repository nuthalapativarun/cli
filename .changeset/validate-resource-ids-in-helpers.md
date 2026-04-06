---
"@googleworkspace/cli": patch
---

Validate resource IDs in docs, sheets, calendar, and drive helpers

`document_id` (docs `+write`), `spreadsheet_id` (sheets `+append` and `+read`),
`calendar_id` (calendar `+insert`), and `parent_id` (drive `+upload`) are now
validated with `validate_resource_name()` before use. This rejects path traversal
segments (`../`), control characters, and URL-special characters (`?`, `#`, `%`)
that could be injected by adversarial AI-agent inputs.
