import java.io.DataInputStream;
import java.nio.file.Path;
import java.util.List;

import net.minecraft.core.BlockPos;
import net.minecraft.nbt.CompoundTag;
import net.minecraft.nbt.NbtIo;
import net.minecraft.world.level.ChunkPos;
import net.minecraft.world.level.chunk.storage.RegionFile;
import net.minecraft.world.level.chunk.storage.RegionStorageInfo;

/**
 * VALIDATION helper for ProbeTreeAttempts: read the vanilla reference region
 * with the jar's own RegionFile/NBT classes and list every LOG block in a
 * chunk window (world coords), so the oracle's surviving trees can be
 * cross-checked against the real world.
 *
 * Usage: ProbeRefLogs <regionDir> <ccx> <ccz>
 */
public class ProbeRefLogs {
    public static void main(String[] args) throws Exception {
        net.minecraft.SharedConstants.tryDetectVersion();
        net.minecraft.server.Bootstrap.bootStrap();
        Path dir = Path.of(args[0]);
        int ccx = Integer.parseInt(args[1]);
        int ccz = Integer.parseInt(args[2]);

        for (int cz = ccz - 1; cz <= ccz + 1; cz++) {
            for (int cx = ccx - 1; cx <= ccx + 1; cx++) {
                int rx = cx >> 5, rz = cz >> 5;
                Path f = dir.resolve("r." + rx + "." + rz + ".mca");
                RegionFile rf = new RegionFile(
                        new RegionStorageInfo("probe",
                                net.minecraft.world.level.Level.OVERWORLD, "probe"),
                        f, Path.of("/tmp/opencode/probelogs"), true);
                try {
                    DataInputStream in = rf.getChunkDataInputStream(new ChunkPos(cx, cz));
                    if (in == null) {
                        System.out.println("chunk " + cx + "," + cz + ": absent");
                        continue;
                    }
                    CompoundTag root = NbtIo.read(in, net.minecraft.nbt.NbtAccounter.unlimitedHeap());
                    in.close();
                    String status = root.getStringOr("Status", "?");
                    System.out.println("chunk " + cx + "," + cz + " status=" + status);
                    var sections = root.getListOrEmpty("sections");

                    for (int si = 0; si < sections.size(); si++) {
                        CompoundTag sec = sections.getCompoundOrEmpty(si);
                        if (!sec.contains("block_states")) continue;
                        CompoundTag bs = sec.getCompoundOrEmpty("block_states");
                        var paletteList = bs.getListOrEmpty("palette");
                        String[] names = new String[paletteList.size()];
                        for (int pi = 0; pi < paletteList.size(); pi++) {
                            names[pi] = paletteList.getCompoundOrEmpty(pi)
                                    .getStringOr("Name", "?");
                        }
                        long[] packedArr = bs.getLongArray("data").orElse(new long[0]);
                        boolean hasLog = false;
                        for (String n : names) {
                            if (n.endsWith("_log")) { hasLog = true; break; }
                        }
                        if (!hasLog || packedArr.length == 0) continue;
                        
                        int yBase = sec.getInt("Y").orElse(0) * 16;
                        // unpack 4-bit indices
                        int idx = 0;
                        int bits = Math.max(4,
                                32 - Integer.numberOfLeadingZeros(names.length - 1));
                        for (int ly = 0; ly < 16; ly++) {
                            for (int lz = 0; lz < 16; lz++) {
                                for (int lx = 0; lx < 16; lx++, idx++) {
                                    int v = getBits(packedArr, idx, bits);
                                    String name = names[v];
                                    if (name.endsWith("_log")) {
                                        System.out.println((cx * 16 + lx) + " "
                                                + (yBase + ly) + " " + (cz * 16 + lz)
                                                + " " + name.replace("minecraft:", ""));
                                    }
                                }
                            }
                        }
                    }
                } finally {
                    rf.close();
                }
            }
        }
    }

    /** Vanilla palette packing: indices do NOT span across long boundaries
     *  (floor(64/bits) values per long). */
    static int getBits(long[] arr, int idx, int bits) {
        int perLong = 64 / bits;
        int off = (idx % perLong) * bits;
        return (int) ((arr[idx / perLong] >>> off) & ((1L << bits) - 1));
    }
}
