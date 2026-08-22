//! Faithful simulation of `java.util.HashSet<BlockPos>` ITERATION ORDER for
//! worldgen parity. Vanilla `TreeFeature.place` collects trunks/foliage in
//! `Sets.newHashSet()` and `TreeDecorator.Context` exposes them as
//! `ObjectArrayList(set)`; the decorator RNG rolls are assigned in THAT
//! order (leaves additionally stable-sorted by Y by the Context ctor), so
//! matching block-for-block requires matching Java's bucket layout.
//!
//! Simulated rules (Java 17+ HashMap):
//!   - hashCode = (y + z*31)*31 + x, wrapping i32 math (`Vec3i.hashCode`,
//!     CFR-verified against the 26.2 sources)
//!   - spread = h ^ (h >>> 16) (unsigned shift)
//!   - default capacity 16, double when ++size > cap*3/4, checked AFTER
//!     the insert
//!   - bucket = spread & (cap - 1); chains append at tail
//!   - resize splits each chain into lo/hi PRESERVING relative order

/// Deduped first-insertion-wins, returned in simulated Java HashSet
/// iteration order.
pub fn java_hash_order(items: Vec<(i32, i32, i32)>) -> Vec<(i32, i32, i32)> {    let mut seen = std::collections::HashSet::with_capacity(items.len());
    let mut uniq: Vec<(i32, i32, i32)> = Vec::with_capacity(items.len());
    for p in items {
        if seen.insert(p) {
            uniq.push(p);
        }
    }
    if uniq.len() < 2 {
        return uniq;
    }
    let spread = |p: (i32, i32, i32)| -> u32 {
        let h: i32 = p
            .1
            .wrapping_add(p.2.wrapping_mul(31))
            .wrapping_mul(31)
            .wrapping_add(p.0);
        (h as u32) ^ ((h as u32) >> 16)
    };
    let mut cap: usize = 16;
    let mut table: Vec<Vec<(i32, i32, i32)>> = vec![Vec::new(); cap];
    let mut size: usize = 0;
    for p in uniq {
        let s = spread(p);
        let b = (s as usize) & (cap - 1);
        if !table[b].contains(&p) {
            table[b].push(p);
            size += 1;
            if size > cap * 3 / 4 {
                let old = std::mem::replace(&mut table, vec![Vec::new(); cap * 2]);
                for chain in old {
                    let mut lo = Vec::new();
                    let mut hi = Vec::new();
                    for q in chain {
                        if (spread(q) as usize) & cap == 0 {
                            lo.push(q);
                        } else {
                            hi.push(q);
                        }
                    }
                    // lo -> new bucket b, hi -> new bucket b + cap (b =
                    // spread & (cap-1)); each receives from exactly one old
                    // bucket, chain order preserved.
                    for q in lo {
                        let nb = (spread(q) as usize) & (cap * 2 - 1);
                        table[nb].push(q);
                    }
                    for q in hi {
                        let nb = (spread(q) as usize) & (cap * 2 - 1);
                        table[nb].push(q);
                    }
                }
                cap *= 2;
            }
        }
    }
    let mut out = Vec::with_capacity(size);
    for chain in &table {
        out.extend(chain.iter().copied());
    }
    out
}
