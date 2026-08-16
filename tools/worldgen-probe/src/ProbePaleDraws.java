import net.minecraft.world.level.levelgen.WorldgenRandom;
import net.minecraft.world.level.levelgen.XoroshiroRandomSource;

/** Ground-truth in_square draw sequence for pale_garden_vegetation (global 13). */
public class ProbePaleDraws {
    public static void main(String[] args) {
        long seed = args.length > 0 ? Long.parseLong(args[0]) : 424242L;
        int cx = args.length > 1 ? Integer.parseInt(args[1]) : 0;
        int cz = args.length > 2 ? Integer.parseInt(args[2]) : 0;
        WorldgenRandom rng = new WorldgenRandom(new XoroshiroRandomSource(0));
        long dec = rng.setDecorationSeed(seed, cx * 16, cz * 16);
        System.out.println("MARKER-A seed=" + seed + " dec=" + dec + " chunk=(" + cx + "," + cz + ")");
        rng.setFeatureSeed(dec, 13, 9);
        for (int i = 0; i < 16; i++) {
            int x = rng.nextInt(16);
            int z = rng.nextInt(16);
            System.out.println("draw " + (i + 1) + " x=" + x + " z=" + z);
        }
        rng.setFeatureSeed(dec, 13, 9);
        for (int i = 0; i < 16; i++) {
            int x = rng.nextInt(16);
            int z = rng.nextInt(16);
            float f1 = rng.nextFloat();
            float f2 = rng.nextFloat();
            System.out.println("MARKER-B sel draw " + (i + 1) + " x=" + x + " z=" + z + " f1=" + f1 + " f2=" + f2);
        }
    }
}