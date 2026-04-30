# frontend

Astro + Bun + Tailwind CSS frontend for FullStackHex.

## Commands

All commands are run from this directory:

| Command                    | Action                                      |
| :------------------------- | :------------------------------------------ |
| `bun install`              | Install dependencies                        |
| `bun run dev`              | Start dev server at `localhost:4321`        |
| `bun run build`            | Build production site to `./dist/`          |
| `bun run preview`          | Preview production build locally            |
| `bun test`                 | Run test suite                              |
| `bun run typecheck`        | Run Astro type checker                      |
| `bun run astro -- --help`  | Other Astro CLI commands                    |

## API Proxy

Server-side routes in `src/pages/api/` proxy requests to the Rust backend.
The frontend never talks to the Python sidecar directly.

## Stack

- Astro (SSR with `@astrojs/node` adapter)
- Tailwind CSS v4 (via `@tailwindcss/vite` Vite plugin)
- TypeScript
- Bun runtime and package manager
