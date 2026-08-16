import net.minecraft.world.level.levelgen.WorldgenRandom;
import net.minecraft.world.level.levelgen.XoroshiroRandomSource;

/** Definitive: print true sequence, then separately reset+consume 4 nextInts and print nextFloat. */
public class ProbeRngReset2 {
    public static void main(String[] args) {
        long seed = Long.parseLong(args[0]);
        WorldgenRandom rng = new WorldgenRandom(new XoroshiroRandomSource(0));
        long dec = rng.setDecorationSeed(seed, 0, 0);
        System.out.println("dec=" + dec);
        // True sequence
        rng.setFeatureSeed(dec, 13, 9);
        StringBuilder sb = new StringBuilder("true: ");
        for (int i = 0; i < 16; i++) {
            sb.append("(").append(rng.nextInt(16)).append(",").append(rng.nextInt(16)).append(") ");
        }
        System.out.println(sb);
        // Reset, consume (x,z), then nextFloat — like the "sel" probe loop
        rng.setFeatureSeed(dec, 13, 9);
        StringBuilder sb2 = new StringBuilder("sel: ");
        for (int i = 0; i < 16; i++) {
            int x = rng.nextInt(16);
            int z = rng.nextInt(16);
            float f1 = rng.nextFloat();
            float f2 = rng.nextFloat();
            sb2.append("(").append(x).append(",").append(z).append(") ");
        }
        System.out.println(sb2);
        // Reset again; consume 2 nextInts only, then nextFloat
        rng.setFeatureSeed(dec, 13, 9);
        rng.nextInt(16);
        rng.nextInt(16);
        System.out.println("float after (3,6): " + rng.nextFloat());
    }
}