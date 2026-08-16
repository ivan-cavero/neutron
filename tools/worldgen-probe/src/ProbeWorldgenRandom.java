import net.minecraft.world.level.levelgen.WorldgenRandom;
import net.minecraft.world.level.levelgen.XoroshiroRandomSource;

/**
 * Golden values for WorldgenRandom wrapping XoroshiroRandomSource.
 * nextLong/nextDouble go through BitRandomSource (two next(bits) calls).
 */
public class ProbeWorldgenRandom {
    public static void main(String[] args) {
        long seed = 12345L;
        int bx = 96;
        int bz = -32;

        WorldgenRandom rng = new WorldgenRandom(new XoroshiroRandomSource(seed));
        rng.setSeed(seed);
        long nl0 = rng.nextLong();
        long nl1 = rng.nextLong();
        System.out.println("after_setSeed nextLong[0]=" + nl0);
        System.out.println("after_setSeed nextLong[1]=" + nl1);

        rng = new WorldgenRandom(new XoroshiroRandomSource(seed));
        long dec = rng.setDecorationSeed(seed, bx, bz);
        System.out.println("decoration=" + dec);

        rng.setFeatureSeed(dec, 0, 6);
        System.out.println("feat0 nextInt(16)=" + rng.nextInt(16));
        System.out.println("feat0 nextInt(16)=" + rng.nextInt(16));
        System.out.println("feat0 nextInt(161)=" + rng.nextInt(161));
        System.out.println("feat0 nextFloat=" + Float.toString(rng.nextFloat()));
        System.out.println("feat0 nextDouble=" + Double.toString(rng.nextDouble()));
        System.out.println("feat0 nextLong=" + rng.nextLong());

        rng = new WorldgenRandom(new XoroshiroRandomSource(seed));
        dec = rng.setDecorationSeed(seed, bx, bz);
        rng.setFeatureSeed(dec, 52, 9);
        System.out.print("feat52_9 ints:");
        for (int i = 0; i < 8; i++) {
            System.out.print(" " + rng.nextInt(16));
        }
        System.out.println();

        rng = new WorldgenRandom(new XoroshiroRandomSource(0x1111222233334444L));
        rng.setSeed(0x1111222233334444L);
        System.out.print("raw next(31) via nextInt(16) x8:");
        for (int i = 0; i < 8; i++) {
            System.out.print(" " + rng.nextInt(16));
        }
        System.out.println();
        System.out.println("raw nextFloat=" + Float.toString(rng.nextFloat()));
        System.out.println("raw nextDouble=" + Double.toString(rng.nextDouble()));
        System.out.println("raw nextLong=" + rng.nextLong());
    }
}
