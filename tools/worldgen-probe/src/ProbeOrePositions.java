import net.minecraft.util.Mth;
import net.minecraft.world.level.levelgen.WorldgenRandom;
import net.minecraft.world.level.levelgen.XoroshiroRandomSource;

/**
 * Replicates vanilla step-6 placed-feature POSITION sampling (modifier chain)
 * for one chunk origin. Prints per-attempt blob origins in FeatureSorter order.
 * args: seed originBlockX originBlockZ
 *
 * Chain model from the 26.2 datapack JSONs:
 *   [rarity_filter: nextFloat < 1/chance]
 *   count: literal N (no draw) | uniform provider min+nextInt(max-min+1)
 *   in_square: nextInt(16), nextInt(16)
 *   height_range uniform: min + nextInt(max-min+1)
 *              trapezoid: plateau=0 -> min + nextInt(end+1) + nextInt(start+1)
 *   heightmap / biome / threshold filters: no draws modeled
 */
public class ProbeOrePositions {
    // name, rarityChance(0 = none), countLiteral(-1 = provider), countMin, countMax,
    // heightType(0 uniform,1 trapezoid,2 heightmap), hMin, hMax
    static final Object[][] F = {
        {"ore_dirt", 0, 7, 0, 0, 0, 0, 160},
        {"ore_gravel", 0, 14, 0, 0, 0, -64, 319},
        {"ore_granite_upper", 6, -2, 0, 0, 0, 64, 128},
        {"ore_granite_lower", 0, 2, 0, 0, 0, 0, 60},
        {"ore_diorite_upper", 6, -2, 0, 0, 0, 64, 128},
        {"ore_diorite_lower", 0, 2, 0, 0, 0, 0, 60},
        {"ore_andesite_upper", 6, -2, 0, 0, 0, 64, 128},
        {"ore_andesite_lower", 0, 2, 0, 0, 0, 0, 60},
        {"ore_tuff", 0, 2, 0, 0, 0, -64, 0},
        {"ore_coal_upper", 0, 30, 0, 0, 0, 136, 319},
        {"ore_coal_lower", 0, 20, 0, 0, 1, 0, 192},
        {"ore_iron_middle", 0, 10, 0, 0, 1, -24, 56},
        {"ore_iron_small", 0, 10, 0, 0, 0, -64, 8},
        {"ore_gold", 0, 4, 0, 0, 1, -64, 32},
        {"ore_gold_lower", 0, -1, 0, 1, 0, -64, -48},
        {"ore_redstone", 0, 4, 0, 0, 0, -64, 15},
        {"ore_redstone_lower", 0, 8, 0, 0, 1, -96, -32},
        {"ore_diamond", 0, 7, 0, 0, 1, -144, 16},
        {"ore_diamond_medium", 0, 2, 0, 0, 0, -64, -4},
        {"ore_diamond_large", 9, -2, 0, 0, 1, -144, 16},
        {"ore_diamond_buried", 0, 4, 0, 0, 1, -144, 16},
        {"ore_lapis", 0, 2, 0, 0, 1, -32, 32},
        {"ore_lapis_buried", 0, 4, 0, 0, 0, -64, 0},
        {"ore_copper_large", 0, 16, 0, 0, 1, -16, 112},
        {"ore_copper", 0, 16, 0, 0, 1, -16, 112},
        {"underwater_magma", 0, -1, 44, 52, 0, -64, 192},
        {"ore_clay", 0, 46, 0, 0, 0, -64, 192},
        {"ore_gold_extra", 0, 50, 0, 0, 0, 32, 256},
        {"disk_grass", 0, 1, 0, 0, 2, 0, 0},
        {"disk_sand", 0, 3, 0, 0, 2, 0, 0},
        {"disk_clay", 0, 1, 0, 0, 2, 0, 0},
        {"disk_gravel", 0, 1, 0, 0, 2, 0, 0},
        {"ore_emerald", 0, 100, 0, 0, 1, -16, 480},
    };

    static int betweenInclusive(WorldgenRandom r, int lo, int hi) {
        return lo + r.nextInt(hi - lo + 1);
    }

    public static void main(String[] args) {
        long seed = Long.parseLong(args[0]);
        int ox = Integer.parseInt(args[1]);
        int oz = Integer.parseInt(args[2]);
        WorldgenRandom rng = new WorldgenRandom(new XoroshiroRandomSource(seed));
        long dec = rng.setDecorationSeed(seed, ox, oz);
        System.out.println("decorationSeed=" + dec);

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

            rng.setFeatureSeed(dec, idx, 6);
            int count;
            if (countLit == -2) {
                // rarity_filter first
                if (!(rng.nextFloat() < 1.0f / rarity)) {
                    System.out.println(idx + " " + name + " RARITY-REJECT");
                    continue;
                }
                count = 1;
            } else if (countLit == -1) {
                count = betweenInclusive(rng, cmin, cmax);
            } else {
                count = countLit;
            }
            for (int a = 0; a < count; a++) {
                if (countLit == -2) {
                    // rarity consumed once per position for provider-less chain?
                    // vanilla: RarityFilter is BEFORE everything and stream starts
                    // with ONE input pos -> single float total. Modeled above.
                }
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
                    y = 0; // heightmap — no draw; real Y from heightmap at place time
                }
                System.out.println(idx + " " + name + " (" + x + "," + y + "," + z + ")");
            }
        }
    }
}
