# mc-decompiler

Download Minecraft server JARs and decompile them with
[Vineflower](https://github.com/Vineflower/vineflower) to read vanilla sources
during parity work.

## Prerequisites

- Rust, Java.
- Vineflower jar at `tools/mc-decompiler/vendor/vineflower.jar` (gitignored) —
  download from the Vineflower releases page. `setup` checks for it.

## Usage

```bash
# Check Java + Vineflower are installed
cargo run -p mc-decompiler -- setup

# Download a server JAR from Mojang (e.g. 26.2)
cargo run -p mc-decompiler -- download 26.2

# Decompile a version (uses the downloaded jar or --jar <path>)
cargo run -p mc-decompiler -- decompile 26.2

# List decompiled versions
cargo run -p mc-decompiler -- list

# Search class names
cargo run -p mc-decompiler -- search Nois

# Show one class
cargo run -p mc-decompiler -- show 26.2 net.minecraft.world.level.levelgen.NoiseGeneratorSettings

# Diff two versions (optionally one class)
cargo run -p mc-decompiler -- diff 26.2 1.21.4
```

Output lands in `tools/mc-decompiler/output/` (gitignored). Mojang JARs are
bundlers — the real server is extracted from `META-INF/versions/` automatically.

## Related

- Replaced the old `vanilla-extract` tool (decompiled dumps were removed from git
  history).
- `worldgen-probe` compiles Java probes against the vanilla jar to verify behavior
  empirically.
