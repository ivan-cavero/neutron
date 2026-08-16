import net.minecraft.world.level.levelgen.WorldgenRandom;
import net.minecraft.world.level.levelgen.XoroshiroRandomSource;

/** Print raw nextLong() streams after identical resets. */
public class ProbeRngReset3 {
    public static void main(String[] args) {
        long seed = Long.parseLong(args[0]);
        WorldgenRandom rng = new WorldgenRandom(new XoroshiroRandomSource(0));
        long dec = rng.setDecorationSeed(seed, 0, 0);
        System.out.println("dec=" + dec);
        rng.setFeatureSeed(dec, 13, 9);
        StringBuilder sb = new StringBuilder("stream A: ");
        for (int i = 0; i < 8; i++) sb.append(rng.nextLong()).append(" ");
        System.out.println(sb);
        rng.setFeatureSeed(dec, 13, 9);
        StringBuilder sb2 = new StringBuilder("stream B: ");
        for (int i = 0; i < 8; i++) sb2.append(rng.nextLong()).append(" ");
        System.out.println(sb2);
        // Now: nextInt(16), nextInt(16), nextFloat, nextFloat pattern with raw longs
        rng.setFeatureSeed(dec, 13, 9);
        StringBuilder sb3 = new StringBuilder("mixed: ");
        for (int i = 0; i < 4; i++) {
            int x = rng.nextInt(16);
            int z = rng.nextInt(16);
            float f1 = rng.nextFloat();
            float f2 = rng.nextFloat();
            sb3.append("(").append(x).append(",").append(z).append(") ");
        }
        System.out.println(sb3);
    }
}