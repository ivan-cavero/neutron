import java.util.BitSet;
import net.minecraft.util.Mth;
import net.minecraft.world.level.levelgen.WorldgenRandom;
import net.minecraft.world.level.levelgen.XoroshiroRandomSource;

/**
 * Stone-blob (OreFeature) two-sided dump aid: replicates the 26.2 modifier
 * chain + OreFeature.place/doPlace math for the six granite/diorite/andesite
 * placed features over a box of chunk origins, printing every touched cell in
 * write order. Uses the REAL jar classes for RNG (WorldgenRandom/Xoroshiro),
 * Mth.sin (table sine) and Mth.lerp/floor, so the only re-implemented parts
 * are control flow copied from OreFeature.java.
 *
 * Assumes the OCEAN_FLOOR_WG bounding-box gate passes (underground blobs).
 *
 * Output:
 *   O <ox> <oz>                 origin header
 *   A <idx> <attempt> <x,y,z>   blob origin (chain draw result)
 *   S <idx> <attempt> <i> <xx> <yy> <zz> <r> <killed0|1>
 *   C <idx> <attempt> <x> <y> <z>
 *
 * args: seed chunkXmin chunkZmin chunkXmax chunkZmax
 * Dumps origins (cx*16, cz*16) for cx in [xmin-1 .. xmax+1], cz likewise.
 */
public class ProbeStoneBlob {
    // gif idx -> {name, rarity(0=count), count, hMin, hMax, size}
    static final Object[][] F = {
        {2, "ore_granite_upper", 6, 1, 64, 128, 64},
        {3, "ore_granite_lower", 0, 2, 0, 60, 64},
        {4, "ore_diorite_upper", 6, 1, 64, 128, 64},
        {5, "ore_diorite_lower", 0, 2, 0, 60, 64},
        {6, "ore_andesite_upper", 6, 1, 64, 128, 64},
        {7, "ore_andesite_lower", 0, 2, 0, 60, 64},
    };

    public static void main(String[] args) {
        long seed = Long.parseLong(args[0]);
        int cx0 = Integer.parseInt(args[1]);
        int cz0 = Integer.parseInt(args[2]);
        int cx1 = Integer.parseInt(args[3]);
        int cz1 = Integer.parseInt(args[4]);

        for (int cz = cz0 - 1; cz <= cz1 + 1; cz++) {
            for (int cx = cx0 - 1; cx <= cx1 + 1; cx++) {
                int ox = cx * 16;
                int oz = cz * 16;
                WorldgenRandom rng = new WorldgenRandom(new XoroshiroRandomSource(seed));
                long dec = rng.setDecorationSeed(seed, ox, oz);
                System.out.println("O " + ox + " " + oz);
                for (Object[] f : F) {
                    int idx = (Integer) f[0];
                    int rarity = (Integer) f[2];
                    int count = (Integer) f[3];
                    int hMin = (Integer) f[4];
                    int hMax = (Integer) f[5];
                    int size = (Integer) f[6];

                    rng.setFeatureSeed(dec, idx, 6);
                    if (rarity > 0) {
                        if (!(rng.nextFloat() < 1.0f / rarity)) continue;
                        count = 1;
                    }
                    for (int a = 0; a < count; a++) {
                        int px = ox + rng.nextInt(16);
                        int pz = oz + rng.nextInt(16);
                        int py = hMin + rng.nextInt(hMax - hMin + 1);
                        System.out.println("A " + idx + " " + a + " " + px + "," + py + "," + pz);

                        // OreFeature.place
                        float dir = rng.nextFloat() * (float) Math.PI;
                        float spreadXY = size / 8.0F;
                        int maxRadius = Mth.ceil((size / 16.0F * 2.0F + 1.0F) / 2.0F);
                        double x0 = px + Math.sin(dir) * spreadXY;
                        double x1 = px - Math.sin(dir) * spreadXY;
                        double z0 = pz + Math.cos(dir) * spreadXY;
                        double z1 = pz - Math.cos(dir) * spreadXY;
                        double y0 = py + rng.nextInt(3) - 2;
                        double y1 = py + rng.nextInt(3) - 2;

                        int fCeil = Mth.ceil(spreadXY);
                        int xStart = px - fCeil - maxRadius;
                        int yStart = py - 2 - maxRadius;
                        int zStart = pz - fCeil - maxRadius;
                        int sizeXZ = 2 * (fCeil + maxRadius);
                        int sizeY = 2 * (2 + maxRadius);

                        // OreFeature.doPlace
                        double[] data = new double[size * 4];
                        for (int i = 0; i < size; i++) {
                            float step = (float) i / size;
                            double xx = Mth.lerp((double) step, x0, x1);
                            double yy = Mth.lerp((double) step, y0, y1);
                            double zz = Mth.lerp((double) step, z0, z1);
                            double ss = rng.nextDouble() * size / 16.0;
                            double r = ((Mth.sin((float) Math.PI * step) + 1.0F) * ss + 1.0) / 2.0;
                            data[i * 4 + 0] = xx;
                            data[i * 4 + 1] = yy;
                            data[i * 4 + 2] = zz;
                            data[i * 4 + 3] = r;
                        }
                        for (int i = 0; i < size; i++) {
                            System.out.println("S " + idx + " " + a + " " + i + " "
                                + data[i * 4 + 0] + " " + data[i * 4 + 1] + " " + data[i * 4 + 2]
                                + " " + data[i * 4 + 3]);
                        }
                        for (int i1 = 0; i1 < size - 1; i1++) {
                            if (!(data[i1 * 4 + 3] <= 0.0)) {
                                for (int i2 = i1 + 1; i2 < size; i2++) {
                                    if (!(data[i2 * 4 + 3] <= 0.0)) {
                                        double dx = data[i1 * 4 + 0] - data[i2 * 4 + 0];
                                        double dy = data[i1 * 4 + 1] - data[i2 * 4 + 1];
                                        double dz = data[i1 * 4 + 2] - data[i2 * 4 + 2];
                                        double dr = data[i1 * 4 + 3] - data[i2 * 4 + 3];
                                        if (dr * dr > dx * dx + dy * dy + dz * dz) {
                                            if (dr > 0.0) {
                                                data[i2 * 4 + 3] = -1.0;
                                            } else {
                                                data[i1 * 4 + 3] = -1.0;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        BitSet tested = new BitSet(sizeXZ * sizeY * sizeXZ);
                        for (int i = 0; i < size; i++) {
                            double r = data[i * 4 + 3];
                            if (!(r < 0.0)) {
                                double xx = data[i * 4 + 0];
                                double yy = data[i * 4 + 1];
                                double zz = data[i * 4 + 2];
                                int xMin = Math.max(Mth.floor(xx - r), xStart);
                                int yMin = Math.max(Mth.floor(yy - r), yStart);
                                int zMin = Math.max(Mth.floor(zz - r), zStart);
                                int xMax = Math.max(Mth.floor(xx + r), xMin);
                                int yMax = Math.max(Mth.floor(yy + r), yMin);
                                int zMax = Math.max(Mth.floor(zz + r), zMin);
                                for (int x = xMin; x <= xMax; x++) {
                                    double xd = (x + 0.5 - xx) / r;
                                    if (xd * xd < 1.0) {
                                        for (int y = yMin; y <= yMax; y++) {
                                            double yd = (y + 0.5 - yy) / r;
                                            if (xd * xd + yd * yd < 1.0) {
                                                for (int z = zMin; z <= zMax; z++) {
                                                    double zd = (z + 0.5 - zz) / r;
                                                    if (xd * xd + yd * yd + zd * zd < 1.0) {
                                                        int bit = x - xStart
                                                            + (y - yStart) * sizeXZ
                                                            + (z - zStart) * sizeXZ * sizeY;
                                                        if (!tested.get(bit)) {
                                                            tested.set(bit);
                                                            System.out.println("C " + idx + " " + a + " " + x + " " + y + " " + z);
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
