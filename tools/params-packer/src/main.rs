// params-packer: deterministic regenerator of
// crates/neutron-worldgen/src/data/biome_params.bin from a transcription of
// vanilla 26.2 OverworldBiomeBuilder.
//
// Usage:
//   params-packer --verify [PATH]   regenerate in memory, byte-compare vs PATH
//                                   (default: ../../crates/neutron-worldgen/src/data/biome_params.bin);
//                                   print PASS/FAIL + first differing record hexdump; exit 0/1.
//   params-packer --emit OUT        write the blob (only on deliberate version bumps).
//
// The blob itself is never touched by --verify; iterate on the transcription
// until it matches byte-for-byte.
//
// Copyright (c) 2026 Neutron Contributors -- MIT License

mod builder;
mod format;

use format::{hexdump, Record, RECORD_SIZE};
use std::path::PathBuf;
use std::process::ExitCode;

const DEFAULT_BLOB: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../crates/neutron-worldgen/src/data/biome_params.bin"
);

const USAGE: &str = "usage: params-packer --verify [PATH]\n       params-packer --emit OUT";

enum Action {
    Verify(Option<PathBuf>),
    Emit(PathBuf),
}

fn parse_args() -> Result<Action, String> {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        Some("--verify") => Ok(Action::Verify(args.next().map(PathBuf::from))),
        Some("--emit") => match args.next() {
            Some(out) => Ok(Action::Emit(PathBuf::from(out))),
            None => Err("--emit requires an OUT path".to_string()),
        },
        Some(other) => Err(format!("unknown argument `{other}`")),
        None => Err("missing mode".to_string()),
    }
}

fn encode_blob(records: &[Record]) -> Vec<u8> {
    let mut blob = Vec::with_capacity(records.len() * RECORD_SIZE);
    for r in records {
        blob.extend_from_slice(&r.encode());
    }
    blob
}

fn run_verify(path: &PathBuf) -> ExitCode {
    let records = builder::build();
    let blob = encode_blob(&records);

    let disk = match std::fs::read(path) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("FAIL: cannot read {}: {e}", path.display());
            return ExitCode::FAILURE;
        }
    };

    if disk.len() != blob.len() {
        println!(
            "FAIL: length mismatch: disk {} bytes vs regenerated {} bytes \
             ({} records x {RECORD_SIZE})",
            disk.len(),
            blob.len(),
            records.len()
        );
        return ExitCode::FAILURE;
    }

    if let Some(byte_idx) = disk.iter().zip(blob.iter()).position(|(a, b)| a != b) {
        let idx = byte_idx / RECORD_SIZE;
        let start = idx * RECORD_SIZE;
        println!(
            "FAIL: first differing record at index {idx} (byte offset {start}):"
        );
        println!("--- disk ---\n{}", hexdump(&disk[start..start + RECORD_SIZE]));
        println!("--- regenerated ---\n{}", hexdump(&blob[start..start + RECORD_SIZE]));
        return ExitCode::FAILURE;
    }

    println!(
        "PASS: byte-identical, all {} records ({} bytes)",
        records.len(),
        blob.len()
    );
    ExitCode::SUCCESS
}

fn run_emit(path: &PathBuf) -> ExitCode {
    let records = builder::build();
    let blob = encode_blob(&records);
    if let Err(e) = std::fs::write(path, &blob) {
        eprintln!("FAIL: cannot write {}: {e}", path.display());
        return ExitCode::FAILURE;
    }
    println!(
        "wrote {}: {} records, {} bytes",
        path.display(),
        records.len(),
        blob.len()
    );
    ExitCode::SUCCESS
}

fn main() -> ExitCode {
    match parse_args() {
        Ok(Action::Verify(path)) => {
            let path = path.unwrap_or_else(|| PathBuf::from(DEFAULT_BLOB));
            run_verify(&path)
        }
        Ok(Action::Emit(path)) => run_emit(&path),
        Err(msg) => {
            eprintln!("{msg}\n{USAGE}");
            ExitCode::FAILURE
        }
    }
}
