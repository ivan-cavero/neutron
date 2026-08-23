import net.minecraft.world.level.levelgen.WorldgenRandom;
import net.minecraft.world.level.levelgen.XoroshiroRandomSource;

/**
 * Step-6 chain simulation WITH blob draw consumption.
 * Exact for discard==0 and discard>=1 features (candidate-cell iteration draws
 * nothing: tag tests draw nothing; shouldSkipAirCheck short-circuits at <=0
 * without drawing and >=1 returns false without drawing).
 * For 0<discard<1 per-cell draws depend on world state -> only attempt 1 is
 * valid, chain stops there (marked).
 *
 * args: seed originX originZ [onlyFeatureIndex]
 */
public class ProbeOreFlow {
    // name, rarity(0=none), countLit(-2 rarity-first,-1 provider), cmin,cmax,
    // htype(0 uniform,1 trapezoid,2 heightmap), hMin,hMax, size, discX100
    static final Object[][] F = {
        {"ore_dirt", 0, 7, 0, 0, 0, 0, 160, 33, 0},
        {"ore_gravel", 0, 14, 0, 0, 0, -64, 319, 33, 0},
        {"ore_granite_upper", 6, -2, 0, 0, 0, 64, 128, 64, 0},
        {"ore_granite_lower", 0, 2, 0, 0, 0, 0, 60, 64, 0},
        {"ore_diorite_upper", 6, -2, 0, 0, 0, 64, 128, 64, 0},
        {"ore_diorite_lower", 0, 2, 0, 0, 0, 0, 60, 64, 0},
        {"ore_andesite_upper", 6, -2, 0, 0, 0, 64, 128, 64, 0},
        {"ore_andesite_lower", 0, 2, 0, 0, 0, 0, 60, 64, 0},
        {"ore_tuff", 0, 2, 0, 0, 0, -64, 0, 64, 0},
        {"ore_coal_upper", 0, 30, 0, 0, 0, 136, 319, 17, 0},
        {"ore_coal_lower", 0, 20, 0, 0, 1, 0, 192, 17, 50},
        {"ore_iron_middle", 0, 10, 0, 0, 1, -24, 56, 9, 0},
        {"ore_iron_small", 0, 10, 0, 0, 0, -64, 8, 4, 0},
        {"ore_gold", 0, 4, 0, 0, 1, -64, 32, 9, 50},
        {"ore_gold_lower", 0, -1, 0, 1, 0, -64, -48, 9, 50},
        {"ore_redstone", 0, 4, 0, 0, 0, -64, 15, 8, 0},
        {"ore_redstone_lower", 0, 8, 0, 0, 1, -96, -32, 8, 0},
        {"ore_diamond", 0, 7, 0, 0, 1, -144, 16, 4, 50},
        {"ore_diamond_medium", 0, 2, 0, 0, 0, -64, -4, 8, 50},
        {"ore_diamond_large", 9, -2, 0, 0, 1, -144, 16, 12, 70},
        {"ore_diamond_buried", 0, 4, 0, 0, 1, -144, 16, 8, 100},
        {"ore_lapis", 0, 2, 0, 0, 1, -32, 32, 7, 0},
        {"ore_lapis_buried", 0, 4, 0, 0, 0, -64, 0, 7, 100},
        {"ore_copper_large", 0, 16, 0, 0, 1, -16, 112, 20, 0},
        {"ore_copper", 0, 16, 0, 0, 1, -16, 112, 10, 0},
        {"underwater_magma", 0, -1, 44, 52, 2, -64, 192, 0, 0},
        {"ore_clay", 0, 46, 0, 0, 0, -64, 192, 33, 0},
        {"ore_gold_extra", 0, 50, 0, 0, 0, 32, 256, 9, 0},
        {"disk_grass", 0, 1, 0, 0, 2, 0, 0, 0, 0},
        {"disk_sand", 0, 3, 0, 0, 2, 0, 0, 0, 0},
        {"disk_clay", 0, 1, 0, 0, 2, 0, 0, 0, 0},
        {"disk_gravel", 0, 1, 0, 0, 2, 0, 0, 0, 0},
        {"ore_emerald", 0, 100, 0, 0, 1, -16, 480, 3, 0},
    };

    static int betweenInclusive(WorldgenRandom r, int lo, int hi) {
        return lo + r.nextInt(hi - lo + 1);
    }

    public static void main(String[] args) {
        long seed = Long.parseLong(args[0]);
        int ox = Integer.parseInt(args[1]);
        int oz = Integer.parseInt(args[2]);
        int only = args.length > 3 ? Integer.parseInt(args[3]) : -1;
        WorldgenRandom rng = new WorldgenRandom(new XoroshiroRandomSource(seed));
        long dec = rng.setDecorationSeed(seed, ox, oz);

        for (int idx = 0; idx < F.length; idx++) {
            Object[] f = F[idx];
            String name = (String) f[0];
            int rarity = (Integer) f[1];
            int countLit = (Integer) f[2];
            int cmin = (Integer) f[3];
            int cmax = (Integer) f[4];
            int htype = (Integer) f[5];
            int hmin = (Integer) f[6];
            int hmax = (Integer) f[7];
            int size = (Integer) f[8];
            int discX100 = (Integer) f[9];

            if (only != -1 && idx != only) continue;
            rng.setFeatureSeed(dec, idx, 6);
            int count;
            if (countLit == -2) {
                if (!(rng.nextFloat() < 1.0f / rarity)) continue;
                count = 1;
            } else if (countLit == -1) {
                count = betweenInclusive(rng, cmin, cmax);
            } else {
                count = countLit;
            }
            boolean worldDependent = discX100 > 0 && discX100 < 100;
            for (int a = 0; a < count; a++) {
                int x = ox + rng.nextInt(16);
                int z = oz + rng.nextInt(16);
                int y;
                if (htype == 0) {
                    y = betweenInclusive(rng, hmin, hmax);
                } else if (htype == 1) {
                    int range = hmax - hmin;
                    if (range <= 0) { y = hmin; }
                    else {
                        int start = range / 2;
                        int end = range - start;
                        y = hmin + betweenInclusive(rng, 0, end) + betweenInclusive(rng, 0, start);
                    }
                } else {
                    y = 0;
                }
                System.out.println(idx + " " + name + " (" + x + "," + y + "," + z + ")");
                rng.nextFloat();
                rng.nextInt(3);
                rng.nextInt(3);
                for (int s = 0; s < size; s++) rng.nextDouble();
                if (worldDependent) {
                    System.out.println("   -- chain WORLD-DEPENDENT after this (discard "
                            + (discX100 / 100.0) + ")");
                    break;
                }
            }
        }
    }
}
