import net.minecraft.world.level.levelgen.WorldgenRandom;
import net.minecraft.world.level.levelgen.XoroshiroRandomSource;

/** Scan step-9 indices: which one yields the observed vanilla tree bases? */
public class ProbeIndexScan {
    public static void main(String[] args) {
        long seed = 12345L;
        int ox = 96;
        int oz = -32;
        int step = 9;
        WorldgenRandom rng = new WorldgenRandom(new XoroshiroRandomSource(seed));
        long dec = rng.setDecorationSeed(seed, ox, oz);
        int[][] wanted = {{8, 8}, {11, 0}, {12, 10}, {5, 14}, {5, 0}, {15, 6}};
        for (int index = 0; index <= 40; index++) {
            rng.setFeatureSeed(dec, index, step);
            java.util.Set<String> rolls = new java.util.HashSet<>();
            for (int i = 0; i < 16; i++) {
                int x = ox + rng.nextInt(16);
                int z = oz + rng.nextInt(16);
                rolls.add((x - ox) + "," + (z - oz));
            }
            int hits = 0;
            for (int[] w : wanted) {
                if (rolls.contains(w[0] + "," + w[1])) hits++;
            }
            if (hits >= 2) {
                System.out.println("index=" + index + " hits=" + hits + " rolls=" + new java.util.TreeSet<>(rolls));
            }
        }
    }
}
