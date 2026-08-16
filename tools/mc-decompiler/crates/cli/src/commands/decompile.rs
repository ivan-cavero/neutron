use std::path::Path;

use mc_decompiler_decompile::pipeline;
use mc_decompiler_store::Store;

pub fn run(version: &str, jar: Option<&Path>, force: bool) -> anyhow::Result<()> {
    let store = Store::open("output")?;

    if force {
        store.remove_version(version)?;
    }

    let jar_path = match jar {
        Some(p) => p.to_path_buf(),
        None => {
            // Try to find downloaded JAR
            let jars_dir = std::path::Path::new("jars");
            let expected = jars_dir.join(format!("server-{version}.jar"));
            if expected.exists() {
                expected
            } else {
                anyhow::bail!(
                    "No JAR specified. Use --jar <path> or run `mc-decompiler download {version}` first"
                );
            }
        }
    };

    let vineflower_jar = find_vineflower()?;
    pipeline::decompile_version(&store, version, &jar_path, &vineflower_jar)?;
    Ok(())
}

fn find_vineflower() -> anyhow::Result<std::path::PathBuf> {
    let cwd = std::env::current_dir().unwrap_or_default();
    let possible = [
        "vendor/vineflower.jar",
        "../vendor/vineflower.jar",
        "../../vendor/vineflower.jar",
        "tools/mc-decompiler/vendor/vineflower.jar",
    ];
    for path in &possible {
        let full = cwd.join(path);
        if full.exists() {
            return Ok(full);
        }
    }
    anyhow::bail!("Vineflower not found. Run setup first")
}
