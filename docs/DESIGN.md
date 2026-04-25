# Docs Design System

Markdown style guide for the bare-metal-demo documentation. All docs in `./docs` follow these rules.

## Header Hierarchy

- `#` = Document title (one per file, line 1)
- `##` = Major section
- `###` = Subsection
- Never skip levels (no `##` directly to `####`)

## Table of Contents

Every doc includes a TOC after the title section. Format:

```
## Table of Contents

1. [Section Name](#section-name)
2. [Another Section](#another-section)
```

Only `INFRASTRUCTURE.md` uses numbered TOC with anchors. All docs now follow this format.

## Table Formatting

- No trailing pipes on table rows
- Align column separators consistently
- Use bold for property/header rows

Example:

```
| Property | Value |
|----------|-------|
| Image    | `postgres:18-alpine` |
| Port     | `5432` |
```

## Code Blocks

Always specify language tag:

- `bash` for shell commands
- `rust` for Rust code
- `python` for Python code
- `typescript` for TypeScript
- `yaml` for Docker Compose / config
- `env` for environment variables
- `json` for JSON responses

## Cross-Linking

- Section name: "Related Docs" (not "Next Steps")
- Position: absolute bottom of file, after all content
- Include previous/next links to guide user journey
- Format: `- [Previous: NAME](./NAME.md)` and `- [Next: NAME](./NAME.md)`

## Text Formatting

- **Bold** for: tool names, ports, file paths, environment variables
- *Italic* for emphasis only
- Use `code` for inline code, commands, URLs

## Callouts

Use GitHub-flavored blockquotes:

```
> **Note:** This is a tip or additional context.
```

```
> **Warning:** This is critical information that needs attention.
```

## Phrasing

- Avoid generic "high performance" or "single source of truth" without stack-specific context
- Use "Rust/Bun/uv stack" or "latest-version stack" for specificity
- Replace "clean, modern" with concrete descriptions
