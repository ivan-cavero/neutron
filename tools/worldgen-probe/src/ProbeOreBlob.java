import net.minecraft.world.level.levelgen.WorldgenRandom;
import net.minecraft.world.level.levelgen.XoroshiroRandomSource;
import net.minecraft.util.Mth;

public class ProbeOreBlob {
    public static void main(String[] args) {
        long seed = 12345L;
        WorldgenRandom rng = new WorldgenRandom(new XoroshiroRandomSource(seed));
        long dec = rng.setDecorationSeed(seed, 96, -32);
        rng.setFeatureSeed(dec, 6, 6);
        rng.nextInt(6); rng.nextInt(16); rng.nextInt(16); rng.nextInt(65);
        int ox = 105, oy = 98, oz = -26;
        float angle = rng.nextFloat() * 3.1415927f;
        float f = 64f / 8f;
        int cell = Mth.ceil((64f / 16f * 2f + 1f) / 2f);
        double sx0 = ox + Math.sin((double) angle) * (double) f;
        double sx1 = ox - Math.sin((double) angle) * (double) f;
        double sz0 = oz + Math.cos((double) angle) * (double) f;
        double sz1 = oz - Math.cos((double) angle) * (double) f;
        double sy0 = oy + rng.nextInt(3) - 2;
        double sy1 = oy + rng.nextInt(3) - 2;
        int size = 64;
        double[] sph = new double[size * 4];
        for (int i = 0; i < size; i++) {
            float t = (float) i / (float) size;
            sph[i*4] = Mth.lerp((double) t, sx0, sx1);
            sph[i*4+1] = Mth.lerp((double) t, sy0, sy1);
            sph[i*4+2] = Mth.lerp((double) t, sz0, sz1);
            double blip = rng.nextDouble() * (double) size / 16.0;
            sph[i*4+3] = ((Mth.sin((double) (3.1415927f * t)) + 1.0f) * blip + 1.0) / 2.0;
        }
        // cull
        for (int i = 0; i < size - 1; i++) {
            if (sph[i*4+3] <= 0) continue;
            for (int j = i + 1; j < size; j++) {
                if (sph[j*4+3] <= 0) continue;
                double dx = sph[i*4]-sph[j*4], dy = sph[i*4+1]-sph[j*4+1], dz = sph[i*4+2]-sph[j*4+2];
                double dr = sph[i*4+3]-sph[j*4+3];
                if (dr*dr > dx*dx+dy*dy+dz*dz) {
                    if (dr > 0) sph[j*4+3] = -1; else sph[i*4+3] = -1;
                }
            }
        }
        int startX = ox - Mth.ceil(f) - cell;
        int startY = oy - 2 - cell;
        int startZ = oz - Mth.ceil(f) - cell;
        int w = 2 * (Mth.ceil(f) + cell);
        int hy = 2 * (2 + cell);
        java.util.BitSet bits = new java.util.BitSet(w * hy * w);
        int count = 0, inChunk = 0;
        int minY = 999, maxY = -999;
        for (int i = 0; i < size; i++) {
            double r = sph[i*4+3];
            if (r < 0) continue;
            double cx = sph[i*4], cy = sph[i*4+1], cz = sph[i*4+2];
            int minx = Math.max(Mth.floor(cx - r), startX);
            int miny = Math.max(Mth.floor(cy - r), startY);
            int minz = Math.max(Mth.floor(cz - r), startZ);
            int maxx = Math.max(Mth.floor(cx + r), minx);
            int maxy = Math.max(Mth.floor(cy + r), miny);
            int maxz = Math.max(Mth.floor(cz + r), minz);
            for (int x = minx; x <= maxx; x++) {
                double dx = ((x + 0.5) - cx) / r;
                if (dx*dx >= 1) continue;
                for (int y = miny; y <= maxy; y++) {
                    double dy = ((y + 0.5) - cy) / r;
                    if (dx*dx+dy*dy >= 1) continue;
                    for (int z = minz; z <= maxz; z++) {
                        double dz = ((z + 0.5) - cz) / r;
                        if (dx*dx+dy*dy+dz*dz >= 1) continue;
                        int bit = (x - startX) + (y - startY) * w + (z - startZ) * w * hy;
                        if (bits.get(bit)) continue;
                        bits.set(bit);
                        count++;
                        if (x >= 96 && x < 112 && z >= -32 && z < -16) {
                            inChunk++;
                            if (y < minY) minY = y;
                            if (y > maxY) maxY = y;
                        }
                    }
                }
            }
        }
        System.out.println("java blob blocks=" + count + " inChunk(6,-2)=" + inChunk + " y=" + minY + ".." + maxY);
        System.out.println("start="+startX+","+startY+","+startZ+" w="+w+" hy="+hy+" cell="+cell);
    }
}
