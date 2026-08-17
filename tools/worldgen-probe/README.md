# worldgen-probe

Java probes that run **against the vanilla 26.2 server jar** to empirically
verify worldgen behavior: RNG sequences, noise instantiation order, feature
sorter indices, sculk/vein placement, etc. Each probe is a `main` class that
prints values the Rust worldgen must reproduce (1:1 parity).

## Build & run

Requires the vanilla 26.2 jar + its libraries (already present under
`tools/nbt-ref/vanilla1/`):

```bash
CP="tools/nbt-ref/vanilla1/versions/26.2/server-26.2.jar:$(find tools/nbt-ref/vanilla1/libraries -name '*.jar' | tr '\n' ':')"

# Compile all probes (output is gitignored)
javac -cp "$CP" -d bin src/*.java

# Run one probe
java -cp "bin:$CP" ProbeNoises 12345
```

`.class` output in `bin/` is gitignored — never commit it.

## Conventions

- Probes take the seed as `args[0]` (some take more args).
- Probe names are self-describing: `ProbeWorldgenRandom`, `ProbeFeatureOrder`,
  `ProbeSculkPatch`, `ProbeAquifer`, …
- Several probes write dump files consumed by Rust examples (e.g.
  `crates/neutron-worldgen/examples/sculk_veintrace.rs` writes
  `tools/worldgen-probe/vein-gate-96--32.txt` for `ProbeSculkVein`).

## Related

- `mc-decompiler` decompiles the same jar to read the sources behind the probes.