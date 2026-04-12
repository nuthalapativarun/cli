---
"@googleworkspace/cli": patch
---

fix(services): add admin-directory alias for the Admin SDK Directory API

`gws admin-reports` implies audit/reporting, not user/group management.
Adds `admin-directory` (and short form `directory`) as dedicated aliases for
`admin/directory_v1`, making the Directory API discoverable without knowing
the internal version string.
