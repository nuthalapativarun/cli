---
"@googleworkspace/cli": minor
---

Add `drive +download` helper for downloading Drive files to a local path

The new `+download` command is a multi-step helper that:
1. Fetches file metadata (name, MIME type) to determine how to download
2. For Google Workspace native files (Docs, Sheets, Slides) uses `files.export`
   with the caller-supplied `--mime-type` (e.g. `application/pdf`, `text/csv`)
3. For all other files uses `files.get?alt=media`
4. Writes the response bytes to a local path validated against path traversal

This complements the existing `+upload` helper and follows all helper
guidelines: it performs multi-step orchestration that the raw Discovery
API cannot express as a single call.

```
gws drive +download --file FILE_ID
gws drive +download --file FILE_ID --output report.pdf
gws drive +download --file FILE_ID --mime-type application/pdf
```
