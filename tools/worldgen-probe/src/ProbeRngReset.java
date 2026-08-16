import net.minecraft.world.level.levelgen.WorldgenRandom;
import net.minecraft.world.level.levelgen.XoroshiroRandomSource;

/** Minimal: does setFeatureSeed reset deterministically after 32 nextInts? */
public class ProbeRngReset {
    public static void main(String[] args) {
        long seed = Long.parseLong(args[0]);
        WorldgenRandom rng = new WorldgenRandom(new XoroshiroRandomSource(0));
        long dec = rng.setDecorationSeed(seed, 0, 0);
        System.out.println("dec=" + dec);
        for (int round = 0; round < 3; round++) {
            rng.setFeatureSeed(dec, 13, 9);
            StringBuilder sb = new StringBuilder("round " + round + ": ");
            for (int i = 0; i < 16; i++) {
                int x = rng.nextInt(16);
                int z = rng.nextInt(16);
                if (i < 6) sb.append("(").append(x).append(",").append(z).append(") ");
            }
            System.out.println(sb);
        }
    }
}