# Provenance

`@liminal-hq/studio-chrome` is a copy of the shared Liminal HQ window chrome. The Studio resolves it as a `link:` dependency from `apps/studio/ui`.

- **Source:** `liminal-hq/waypoint`, `packages/chrome`
- **Commit:** `a3c9894728da69a9b44e66310e66cde64c10ea60` (the package's own last change is `4aa5daea16725ad3d1eea44c494e3b9036f9f0ad`)
- **Licence:** `Apache-2.0 OR MIT`, as upstream. Both licence files are copied alongside.

## Changes

`scripts/check-headers.sh` skips `vendor/`, because the copy keeps upstream's files as they are (some have no header).

The only change to `src/` is mechanical: `--wp-` becomes `--lic-` and `data-wp-` becomes `data-lic-`, so the chrome's custom properties and attributes cannot collide with an app's own tokens (Planboard does the same with `--pbc-`). Everything Studio-specific lives in app code that uses the chrome's props and slots.

Left out: the tests and test setup (`src/**/*.test.*`, `src/test/`), `src/ContextMenu/expectEveryItemHasIcon.ts` (a test helper), `tsconfig.json` and `vitest.config.ts` (the Studio's own `tsconfig.json` covers the copy), and the dev dependencies and scripts in `package.json`. The package is renamed to `@liminal-hq/studio-chrome`, and `package.json` drops the two exports that point at removed files. `README.md` is upstream's with the same renames.

## Refresh

1. Check out the commit to take in the Waypoint repository.
2. Replace `src/` here with `packages/chrome/src`, then delete `*.test.*` files, `src/test/` and `src/ContextMenu/expectEveryItemHasIcon.ts`.
3. Re-run the rename: `grep -rlE "wp-" src README.md | xargs sed -i -E 's/--wp-/--lic-/g; s/data-wp-/data-lic-/g'`.
4. Copy the licence files, update the commit above, and add any new files to the `exports` map in `package.json`.
5. Run `bun run typecheck && bun run test:js && bun run build` from the repository root.

Do not edit the files in place. A fix belongs upstream and comes back through a refresh.
