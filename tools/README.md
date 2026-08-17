# tools — index

The parity pipeline has two sides: the **reference** (what vanilla generated) and
the **neutron** side (what we generate). Everything else is support tooling.

```text
vanilla server ──> ref-extract ──> reference JSON ──┐
                                                    ├──> ref-extract compare ──> parity verdict
neutron-worldgen ─> neutron-hash ──> neutron JSON ──┘
```

## The parity pipeline

| Tool | Side | What it does | When to use it |
| --- | --- | --- | --- |
| `ref-extract` | reference | Boots vanilla/paper/folia servers, hashes every chunk of the generated world into a JSON (`extract` / `extract-all`), and **compares** two JSONs (`compare`) | Produce the vanilla bar, or check hash-level parity (fast gate, seeds × servers) |
| `neutron-hash` | neutron | Generates chunks with neutron-worldgen, prints stats, emits the neutron JSON (`--generate-neutron`) | Produce the neutron side before running `ref-extract compare` |

## Block-level parity (the real bar)

The worldgen examples compare **block by block** against real vanilla region
files (`tools/nbt-ref/`), reporting recall/precision per feature — this is what
the runs measure:

- `region_parity` — full chunk region parity
- `clay_overlap` — clay placement parity
- `lush_pale_parity` — lush caves / pale garden placement parity

Run them from `crates/neutron-worldgen` (see `runs/run-046.md` for args).

## Support tooling

| Tool | What it does | When to use it |
| --- | --- | --- |
| `chunk-dump` | Prints the full NBT tree of one chunk from a `.mca` | Debugging *why* a chunk differs (the "lupa" after `compare` says it differs) |
| `mc-decompiler` | Downloads + decompiles server jars with Vineflower | Reading vanilla sources during parity work |
| `worldgen-probe` | Java probes run against the vanilla jar (RNG, noise, feature order…) | Verifying a specific vanilla behavior empirically |
| `nbt-ref` | A real vanilla 26.2 server + generated world (gitignored) | The ground truth the examples read; also the classpath for probes |

## Mental model

- **Hash level** (fast, multi-seed): `ref-extract extract-all` → `neutron-hash --generate-neutron` → `ref-extract compare`
- **Block level** (the bar, per feature): the `region_parity`/`clay_overlap`/`lush_pale_parity` examples
- **Why does it differ?**: `chunk-dump` to inspect, `mc-decompiler` + `worldgen-probe` to understand vanilla behavior