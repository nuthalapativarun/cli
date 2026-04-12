# gws-docs Cookbook

Hand-written recipes for non-obvious Google Docs patterns. Use alongside `SKILL.md`.

---

## 1. Tab CRUD Operations

### Read a specific tab

By default `documents.get` returns the document body without tab content. Pass `includeTabsContent=true` to get all tabs, then look for the tab by `tabId`:

```bash
gws docs documents get \
  --params '{"documentId":"DOC_ID","includeTabsContent":true}'
```

The response nests content under `tabs[].documentTab.body`.

### List tabs in a document

```bash
gws docs documents get \
  --params '{"documentId":"DOC_ID","includeTabsContent":false}' \
| jq '.tabs[] | {tabId: .tabProperties.tabId, title: .tabProperties.title}'
```

### Create a tab

Use `addDocumentTab` (not `createTab` — that request type does not exist):

```bash
gws docs documents batchUpdate \
  --params '{"documentId":"DOC_ID"}' \
  --json '{
    "requests": [{
      "addDocumentTab": {
        "documentTab": {},
        "insertionIndex": 1
      }
    }]
  }'
```

`insertionIndex` is 0-based; omit it to append the tab at the end.

### Write to a specific tab

Include `tabId` in the `location` or `range` of every request. Without it, writes go to the first tab:

```bash
gws docs documents batchUpdate \
  --params '{"documentId":"DOC_ID"}' \
  --json '{
    "requests": [{
      "insertText": {
        "text": "Hello from tab 2",
        "location": {
          "segmentId": "",
          "index": 1,
          "tabId": "TAB_ID"
        }
      }
    }]
  }'
```

### Rename a tab

```bash
gws docs documents batchUpdate \
  --params '{"documentId":"DOC_ID"}' \
  --json '{
    "requests": [{
      "updateDocumentTab": {
        "documentTab": {
          "tabProperties": {
            "tabId": "TAB_ID",
            "title": "New Name"
          }
        },
        "tabUpdateMask": "tabProperties.title"
      }
    }]
  }'
```

### Delete a tab

```bash
gws docs documents batchUpdate \
  --params '{"documentId":"DOC_ID"}' \
  --json '{
    "requests": [{
      "deleteDocumentTab": {
        "tabId": "TAB_ID"
      }
    }]
  }'
```

---

## 2. Formatted Content Insertion

The pattern is always: **insert text first, then apply styles**. Styles reference character offsets, so the text must exist before styling.

### Insert a heading

```bash
gws docs documents batchUpdate \
  --params '{"documentId":"DOC_ID"}' \
  --json '{
    "requests": [
      {
        "insertText": {
          "text": "My Heading\n",
          "location": {"segmentId": "", "index": 1}
        }
      },
      {
        "updateParagraphStyle": {
          "range": {"segmentId": "", "startIndex": 1, "endIndex": 12},
          "paragraphStyle": {"namedStyleType": "HEADING_1"},
          "fields": "namedStyleType"
        }
      }
    ]
  }'
```

Named style types: `HEADING_1` through `HEADING_6`, `NORMAL_TEXT`, `TITLE`, `SUBTITLE`.

### Bold / italic text

Apply `updateTextStyle` after inserting the text:

```bash
{
  "updateTextStyle": {
    "range": {"segmentId": "", "startIndex": 1, "endIndex": 6},
    "textStyle": {"bold": true, "italic": false},
    "fields": "bold,italic"
  }
}
```

### Insert a bulleted list item

```bash
[
  {
    "insertText": {
      "text": "List item\n",
      "location": {"segmentId": "", "index": 1}
    }
  },
  {
    "createParagraphBullets": {
      "range": {"segmentId": "", "startIndex": 1, "endIndex": 11},
      "bulletPreset": "BULLET_DISC_CIRCLE_SQUARE"
    }
  }
]
```

---

## 3. Valid `batchUpdate` Request Types (reference)

| Request type | Purpose |
|---|---|
| `insertText` | Insert plain text at a location |
| `insertInlineImage` | Embed an image by URI |
| `deleteContentRange` | Remove a range of content |
| `updateParagraphStyle` | Set heading level, alignment, spacing |
| `updateTextStyle` | Bold, italic, font size, colour |
| `createParagraphBullets` | Add list bullets/numbering |
| `deleteParagraphBullets` | Remove bullets |
| `insertTable` | Create a table |
| `insertTableRow` / `insertTableColumn` | Add rows/columns |
| `deleteTableRow` / `deleteTableColumn` | Remove rows/columns |
| `addDocumentTab` | Create a new tab |
| `updateDocumentTab` | Rename or reorder a tab |
| `deleteDocumentTab` | Delete a tab |
| `replaceAllText` | Find-and-replace across the document |
| `createNamedRange` | Tag a range with a name for later reference |
| `deleteNamedRange` | Remove a named range |

Run `gws schema docs.documents.batchUpdate` to see the full schema with all fields.

---

## 4. `+write` Helper Limitations

`gws docs +write` is a convenience helper for the common case of **appending plain text** to a document. It has intentional constraints:

| Limitation | Workaround |
|---|---|
| Plain text only — no headings, bold, or lists | Use `documents.batchUpdate` directly with formatting requests |
| Always appends to end of document body | Specify `location.index` in a raw `insertText` request |
| No tab targeting — writes to first tab | Use `insertText` with `location.tabId` (see §1 above) |
| No image insertion | Use `insertInlineImage` via `documents.batchUpdate` |

For simple append-only tasks `+write` is fine. For anything involving structure or tabs, reach for the raw API.
