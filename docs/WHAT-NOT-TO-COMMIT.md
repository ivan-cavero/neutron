# Qué no subir a git

Si el IDE muestra decenas de miles de “cambios”, casi seguro son **archivos ignorados** (build + vanilla extract), no código sin commit.

Comprobar:

```bash
git status                 # debe decir "working tree clean" si no hay trabajo real
git status --ignored -sb   # lista ruido ignorado
git ls-files --others --ignored --exclude-standard | measure  # ~60k en dev local
```

## Nunca commits

| Ruta | Por qué |
|------|---------|
| `target/`, `**/target/` | Build Rust |
| `tools/vanilla-extract/server-classes/` | Unpack del server jar (~15k archivos) |
| `tools/**/**/*.jar` | Binarios Mojang |
| `**/world/` | Mundos generados |
| `bench/results/*.json` | Salidas de bench |
| `logs/`, `*.log` | Logs |

## Sí se commitea

- `crates/**` (incluye `neutron-worldgen/src/data/worldgen` JSON de biomas/features)
- `tools/**/*.rs`, `*.ps1`, `*.md`, `decompiled/**/*.java`
- `runs/`, `tests/` (fuentes), `AGENTS.md`, `STATE.md`

## Regenerar extract local

```powershell
pwsh tools/vanilla-extract/extract-worldgen.ps1
```
