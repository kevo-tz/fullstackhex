# AGENTS.md

## Health Stack

- typecheck: cd frontend && tsc --noEmit
- lint: cd backend && cargo clippy -- -D warnings
- test: cd backend && cargo test
- test: cd frontend && vitest run
- test: cd frontend && bun test
- test: cd py-api && pytest
- deadcode: cd frontend && npx knip
- shell: shellcheck -x scripts/*.sh tests/*.sh

## Skill routing

When the user's request matches an available skill, invoke it via the Skill tool. When in doubt, invoke the skill.

Key routing rules:
- Product ideas/brainstorming → invoke /office-hours
- Strategy/scope → invoke /plan-ceo-review
- Architecture → invoke /plan-eng-review
- Design system/plan review → invoke /design-consultation or /plan-design-review
- Full review pipeline → invoke /autoplan
- Bugs/errors → invoke /investigate
- QA/testing site behavior → invoke /qa or /qa-only
- Code review/diff check → invoke /review
- Visual polish → invoke /design-review
- Ship/deploy/PR → invoke /ship or /land-and-deploy
- Save progress → invoke /context-save
- Resume context → invoke /context-restore
