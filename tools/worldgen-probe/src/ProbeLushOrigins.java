import net.minecraft.world.level.levelgen.WorldgenRandom;
import net.minecraft.world.level.levelgen.XoroshiroRandomSource;

/**
 * Pre-scan attempt origins for the lush_caves step-9 features of one chunk.
 *
 * Mirrors PlacedFeature.placeWithContext modifier order for:
 *   idx 27 lush_caves_ceiling_vegetation  count 125  offset y-1
 *   idx 28 cave_vines                     count 188  offset y-1
 *   idx 29 lush_caves_clay                count 62   offset y+1
 *   idx 30 lush_caves_vegetation          count 125  offset y+1
 *
 * Per attempt: nextInt(16) x, nextInt(16) z (in_square),
 * then height uniform [-64..256] = -64 + nextInt(321).
 * environment_scan consumes no RNG and runs BEFORE random_offset.
 *
 * Usage: ProbeLushOrigins <seed> <chunkX> <chunkZ>
 */
public class ProbeLushOrigins {
    public static void main(String[] args) {
        long seed = args.length > 0 ? Long.parseLong(args[0]) : 424242L;
        int cx = args.length > 1 ? Integer.parseInt(args[1]) : 0;
        int cz = args.length > 2 ? Integer.parseInt(args[2]) : -1;
        int[][] feats = {{27, 125}, {28, 188}, {29, 62}, {30, 125}};
        for (int[] f : feats) {
            WorldgenRandom rng = new WorldgenRandom(new XoroshiroRandomSource(0));
            long dec = rng.setDecorationSeed(seed, cx * 16, cz * 16);
            rng.setFeatureSeed(dec, f[0], 9);
            System.out.println("== feature " + f[0] + " count " + f[1] + " dec=" + dec
                + " originBlock=(" + (cx * 16) + "," + (cz * 16) + ")");
            for (int i = 0; i < f[1]; i++) {
                int lx = rng.nextInt(16);
                int lz = rng.nextInt(16);
                int y = -64 + rng.nextInt(321);
                System.out.println("draw " + (i + 1) + " lx=" + lx + " lz=" + lz + " y_pre_scan=" + y);
            }
        }
    }
}
