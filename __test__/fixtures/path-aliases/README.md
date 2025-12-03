# Path Aliases Fixture

This fixture tests path alias resolution (e.g., `@` for `src`, `~` for `lib`).

## Structure

- `src/utils.ts` - Simple exports with one unused
- `src/lib/helpers/deep.ts` - Deeply nested file for testing nested alias resolution
- `lib/helpers.ts` - File in a different aliased directory

## Test Cases

- Basic alias: `@` → `src`
- Nested path: `@/lib/helpers/deep`
- Multiple aliases: `@` → `src`, `~` → `lib`
