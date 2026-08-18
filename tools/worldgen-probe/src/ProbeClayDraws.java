import net.minecraft.world.level.levelgen.WorldgenRandom;
import net.minecraft.world.level.levelgen.XoroshiroRandomSource;

/** lush_caves_clay (global 29, step 9) draw stream for seed 424242 chunk (0,0). */
public class ProbeClayDraws {
    public static void main(String[] args) {
        long seed = args.length > 0 ? Long.parseLong(args[0]) : 424242L;
        int cx = args.length > 1 ? Integer.parseInt(args[1]) : 0;
        int cz = args.length > 2 ? Integer.parseInt(args[2]) : 0;
        WorldgenRandom rng = new WorldgenRandom(new XoroshiroRandomSource(0));
        long dec = rng.setDecorationSeed(seed, cx * 16, cz * 16);
        rng.setFeatureSeed(dec, 29, 9);
        for (int i = 0; i < 62; i++) {
            int x = rng.nextInt(16);
            int z = rng.nextInt(16);
            int y = -64 + rng.nextInt(321);
            System.out.println("draw " + (i + 1) + " x=" + x + " z=" + z + " y=" + y);
        }
    }
}
