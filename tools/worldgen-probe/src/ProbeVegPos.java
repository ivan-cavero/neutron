import net.minecraft.world.level.levelgen.WorldgenRandom;
import net.minecraft.world.level.levelgen.XoroshiroRandomSource;

/** 16 in_square xz for dark_forest_vegetation (FeatureSorter index 17, step 9). */
public class ProbeVegPos {
    public static void main(String[] args) {
        long seed = 12345L;
        int ox = 96;
        int oz = -32;
        int index = 17;
        int step = 9;
        WorldgenRandom rng = new WorldgenRandom(new XoroshiroRandomSource(seed));
        long dec = rng.setDecorationSeed(seed, ox, oz);
        rng.setFeatureSeed(dec, index, step);
        System.out.println("dec=" + dec);
        for (int i = 0; i < 16; i++) {
            int x = ox + rng.nextInt(16);
            int z = oz + rng.nextInt(16);
            System.out.println("i=" + i + " (" + x + "," + z + ") local=(" + (x - ox) + "," + (z - oz) + ")");
        }
    }
}
