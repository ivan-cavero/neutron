// chunk-dump: Dump vanilla chunk NBT structure from .mca region files.
//
// Usage:
//   cargo run -p chunk-dump -- <region.mca> <local_cx> <local_cz> [--dump-longs] [--verbose]
//
// Reads one chunk from an Anvil region file and prints the full NBT tree,
// including palette entries and (optionally) the raw packed long arrays.

use std::path::PathBuf;

use anyhow::{Context, Result};
use neutron_world::nbt::ussr_nbt::mutf8::MString;
use neutron_world::nbt::ussr_nbt::owned::{Compound, List, Nbt, Tag};
use neutron_world::{parse_region_filename, Region};

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 4 {
        eprintln!(
            "Usage: chunk-dump <region.mca> <local_cx> <local_cz> [--dump-longs] [--verbose]"
        );
        std::process::exit(1);
    }
    let mca_path = PathBuf::from(&args[1]);
    let cx: i32 = args[2].parse()?;
    let cz: i32 = args[3].parse()?;
    let dump_longs = args.iter().any(|a| a == "--dump-longs");
    let verbose = args.iter().any(|a| a == "--verbose");

    let (rx, rz) = parse_region_filename(
        mca_path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown"),
    )
    .context("bad region filename")?;
    println!(
        "Region file: {} (region {}, {})",
        mca_path.display(),
        rx,
        rz
    );
    println!(
        "Chunk local coords: ({}, {}) -> global ({}, {})",
        cx,
        cz,
        rx * 32 + cx,
        rz * 32 + cz
    );

    let region = Region::open(&mca_path)?.with_coords(rx, rz);
    let data = region
        .get_chunk(cx, cz)?
        .context("chunk not present in region")?;
    println!("Decompressed chunk size: {} bytes", data.len());

    let nbt = neutron_world::nbt::read_nbt(&data)?;
    dump_nbt(&nbt, 0, dump_longs, verbose);

    Ok(())
}

fn dump_nbt(nbt: &Nbt, indent: usize, dump_longs: bool, verbose: bool) {
    println!(
        "{}root: {}",
        "  ".repeat(indent),
        tag_type(nbt.compound.tags.len())
    );
    for (name, tag) in &nbt.compound.tags {
        dump_tag(name, tag, indent + 1, dump_longs, verbose);
    }
}

fn tag_type(_n: usize) -> &'static str {
    "compound"
}

fn dump_tag(name: &MString, tag: &Tag, indent: usize, dump_longs: bool, verbose: bool) {
    let pad = "  ".repeat(indent);
    match tag {
        Tag::Byte(v) => println!("{}{}: byte {}", pad, name, v),
        Tag::Short(v) => println!("{}{}: short {}", pad, name, v),
        Tag::Int(v) => println!("{}{}: int {}", pad, name, v),
        Tag::Long(v) => println!("{}{}: long {}", pad, name, v),
        Tag::Float(v) => println!("{}{}: float {}", pad, name, v),
        Tag::Double(v) => println!("{}{}: double {}", pad, name, v),
        Tag::String(s) => println!("{}{}: string {:?}", pad, name, s),
        Tag::ByteArray(v) => {
            println!("{}{}: byte_array len={}", pad, name, v.len());
            if verbose {
                println!("{}  {:?}", pad, v);
            }
        }
        Tag::IntArray(v) => {
            println!("{}{}: int_array len={}", pad, name, v.len());
            if verbose {
                let vals: Vec<i32> = v.to_vec();
                println!("{}  {:?}", pad, vals);
            }
        }
        Tag::LongArray(v) => {
            println!("{}{}: long_array len={}", pad, name, v.len());
            if dump_longs {
                let vals: Vec<i64> = v.to_vec();
                println!("{}  {:?}", pad, vals);
            }
        }
        Tag::List(list) => {
            println!(
                "{}{}: list<{}> len={}",
                pad,
                name,
                list_elem_type(list),
                list_len(list)
            );
            dump_list(list, indent, dump_longs, verbose);
        }
        Tag::Compound(c) => {
            println!("{}{}: compound", pad, name);
            dump_compound(c, indent, dump_longs, verbose);
        }
    }
}

fn dump_compound(c: &Compound, indent: usize, dump_longs: bool, verbose: bool) {
    for (n, t) in &c.tags {
        dump_tag(n, t, indent + 1, dump_longs, verbose);
    }
}

fn dump_list(list: &List, indent: usize, dump_longs: bool, verbose: bool) {
    let pad = "  ".repeat(indent + 1);
    match list {
        List::Empty => {}
        List::Byte(v) => println!("{}{:?}", pad, v),
        List::String(v) => {
            for s in v {
                println!("{}{:?}", pad, s);
            }
        }
        List::Compound(items) => {
            for item in items {
                dump_compound(item, indent + 1, dump_longs, verbose);
                println!();
            }
        }
        List::Int(v) => {
            let vals: Vec<i32> = v.to_vec();
            println!("{}{:?}", pad, vals);
        }
        List::Long(v) => {
            let vals: Vec<i64> = v.to_vec();
            println!("{}{:?}", pad, vals);
        }
        List::Short(v) => {
            let vals: Vec<i16> = v.to_vec();
            println!("{}{:?}", pad, vals);
        }
        List::Float(v) => {
            let vals: Vec<f32> = v.to_vec();
            println!("{}{:?}", pad, vals);
        }
        List::Double(v) => {
            let vals: Vec<f64> = v.to_vec();
            println!("{}{:?}", pad, vals);
        }
        List::ByteArray(v) => println!("{}{:?}", pad, v),
        List::List(v) => {
            for item in v {
                dump_list(item, indent + 1, dump_longs, verbose);
            }
        }
        List::IntArray(v) => println!("{}{:?}", pad, v),
        List::LongArray(v) => println!("{}{:?}", pad, v),
    }
}

fn list_len(list: &List) -> usize {
    match list {
        List::Empty => 0,
        List::Byte(v) => v.len(),
        List::Short(v) => v.len(),
        List::Int(v) => v.len(),
        List::Long(v) => v.len(),
        List::Float(v) => v.len(),
        List::Double(v) => v.len(),
        List::ByteArray(v) => v.len(),
        List::String(v) => v.len(),
        List::List(v) => v.len(),
        List::Compound(v) => v.len(),
        List::IntArray(v) => v.len(),
        List::LongArray(v) => v.len(),
    }
}

fn list_elem_type(list: &List) -> &'static str {
    match list {
        List::Empty => "end",
        List::Byte(_) => "byte",
        List::Short(_) => "short",
        List::Int(_) => "int",
        List::Long(_) => "long",
        List::Float(_) => "float",
        List::Double(_) => "double",
        List::ByteArray(_) => "byte_array",
        List::String(_) => "string",
        List::List(_) => "list",
        List::Compound(_) => "compound",
        List::IntArray(_) => "int_array",
        List::LongArray(_) => "long_array",
    }
}
