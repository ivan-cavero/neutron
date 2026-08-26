// params-packer/format.rs: record encoding for the packed climate blob.
//
// Blob layout (little-endian, no header): N records x 97 bytes. Per record:
// u8 biome id (Neutron-internal ids, see neutron-worldgen biome/source.rs)
// + 12 x i64 = [t.min, t.max, h.min, h.max, c.min, c.max, e.min, e.max,
//               d.min, d.max, w.min, w.max].
//
// The vanilla offset column is omitted: every overworld emission passes
// offset 0.0F, so the field would be all zeros; there is no offset slot here
// by construction (asserted nowhere at runtime because nothing can carry it).
//
// Copyright (c) 2026 Neutron Contributors -- MIT License

/// Bytes per record: `u8` biome id + 12 × `i64`.
pub const RECORD_SIZE: usize = 1 + 12 * 8;

/// One climate parameter point: biome id + 6 `[min, max]` quantized intervals.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Record {
    /// Neutron-internal biome id (see `biome_id` consts in neutron-worldgen).
    pub biome: u8,
    /// `[t_min, t_max, h_min, h_max, c_min, c_max, e_min, e_max, d_min, d_max, w_min, w_max]`.
    pub intervals: [i64; 12],
}

impl Record {
    /// Encode to the 97-byte little-endian on-disk form.
    pub fn encode(&self) -> [u8; RECORD_SIZE] {
        let mut out = [0u8; RECORD_SIZE];
        out[0] = self.biome;
        for (i, v) in self.intervals.iter().enumerate() {
            let start = 1 + i * 8;
            out[start..start + 8].copy_from_slice(&v.to_le_bytes());
        }
        out
    }
}

/// Fixed-width hexdump of one record (97 bytes), 16 bytes per line.
pub fn hexdump(bytes: &[u8]) -> String {
    assert_eq!(bytes.len(), RECORD_SIZE, "hexdump expects one full record");
    let mut s = String::with_capacity(bytes.len() * 4);
    for chunk in bytes.chunks(16) {
        for b in chunk {
            s.push_str(&format!("{b:02x} "));
        }
        s.push('\n');
    }
    // strip the trailing newline of the last line
    s.truncate(s.len() - 1);
    s
}
