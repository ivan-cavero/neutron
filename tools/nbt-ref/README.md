# nbt-ref

Vanilla 26.2 reference data — the *bar* for worldgen parity. The `vanilla1/`
directory is a full vanilla server instance (gitignored) whose generated world is
the ground truth that neutron-worldgen must reproduce byte-for-byte.

## What's in it

- `vanilla1/server.jar` + `versions/26.2/server-26.2.jar` — the vanilla server
  (bundler). Also the classpath target for `worldgen-probe`.
- `vanilla1/libraries/` — Mojang runtime libraries (gitignored).
- `vanilla1/world/` — generated reference world: regions, `level.dat`, POI,
  entities. This is what `ref-extract` hashes and what the worldgen examples
  (`region_parity`, `clay_overlap`, `lush_pale_parity`) read.

## Re-extracting

The whole directory is gitignored and re-creatable:

```bash
# 1. Provision a fresh server (jar + eula + server.properties with a fixed seed)
# 2. Boot it, let it generate chunks (marker: "Done (Xs)!"), stop it
# 3. The world lands in world/dimensions/minecraft/overworld/region/
```

For multi-seed reference *hashes* (not worlds), use `ref-extract extract-all`
instead — it boots throwaway servers and hashes the chunks, no committed worlds.

## Related

- `ref-extract` — hash extraction + comparison against these worlds.
- `chunk-dump` — inspect a single chunk's NBT from these region files.