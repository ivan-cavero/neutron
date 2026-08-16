import net.minecraft.world.level.levelgen.WorldgenRandom;
import net.minecraft.world.level.levelgen.XoroshiroRandomSource;

/** Print underlying nextLong stream AND nextInt(16) sequence after reset. */
public class ProbeRngReset4 {
    public static void main(String[] args) {
        long seed = Long.parseLong(args[0]);
        WorldgenRandom rng = new WorldgenRandom(new XoroshiroRandomSource(0));
        long dec = rng.setDecorationSeed(seed, 0, 0);
        System.out.println("dec=" + dec);
        rng.setFeatureSeed(dec, 13, 9);
        StringBuilder sb = new StringBuilder("underlying: ");
        for (int i = 0; i < 40; i++) sb.append(rng.nextLong()).append(" ");
        System.out.println(sb);
        rng.setFeatureSeed(dec, 13, 9);
        StringBuilder sb2 = new StringBuilder("nextInt16: ");
        for (int i = 0; i < 20; i++) sb2.append(rng.nextInt(16)).append(" ");
        System.out.println(sb2);
    }
}