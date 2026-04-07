# Metadata templating layout investigation

This note records the investigation behind Covermint's metadata templating system for top/left cover-adjacent sections.

## Requirements considered

- Placeholders: `{{artist}}`, `{{title}}`, `{{album}}`, `{{trackNumber}}`, `{{length}}`
- Escape line breaks with `\n`
- Modifiers for truncation intent: `:start`, `:end`
- Two layout sections: `top` and `left`
- If only one section exists, it must still align to the cover bounds
- Alignment modifiers (`start` / `end`) are desirable

## Options evaluated

### 1) Full template engine crate (e.g. Handlebars/Tera)

Pros:
- Rich expression support
- Familiar syntax

Cons:
- Adds heavy dependency surface for a small runtime text-substitution problem
- More escaping/logic complexity than needed
- Harder to constrain safely for tiny config snippets

Decision: not selected.

### 2) Pango markup as the template language

Pros:
- Already supported by GTK labels
- Styling inline is possible

Cons:
- Placeholder parsing is still custom work
- Easy to break rendering with malformed markup
- Mixes content templating with presentation too early

Decision: not selected as primary model.

### 3) Minimal custom placeholder renderer (selected)

Pros:
- Tiny and predictable behavior
- Exact control over supported fields/modifiers
- Easy to validate and provide fallbacks for bad config values
- Matches current needs without introducing a logic language

Cons:
- Feature growth requires explicit implementation
- No conditionals/loops/macros

Decision: selected.

## Chosen model

- Templates are plain strings with placeholders in `{{...}}`
- Supported fields:
  - `artist`
  - `title`
  - `album`
  - `trackNumber`
  - `length` (formatted `min:sec`)
- Optional field modifier:
  - `:start`
  - `:end`
- `\n` in config is converted to real line breaks at render time
- Per-section config:
  - `top`
  - `left`
- Per-section alignment:
  - `align = "start" | "end"`

Malformed templates are reported on startup and fall back to safe defaults.

## Why this fits current scope

The selected model is intentionally small, deterministic, and aligned with the current GTK overlay architecture. It supports all required metadata placeholders and layout sections while keeping runtime behavior easy to reason about.
