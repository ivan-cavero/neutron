import net.minecraft.util.Mth;
import net.minecraft.world.level.levelgen.WorldgenRandom;
import net.minecraft.world.level.levelgen.XoroshiroRandomSource;

/**
 * Replicate vanilla step-6 placed-feature position sampling for ONE chunk
 * origin: count -> in_square -> height_range (uniform | trapezoid).
 * Prints blob origins in order per feature index.
 * args: seed cx cz
 */
public class ProbeOrePositions {
    // index -> (count, height type, min, max)  [min/max already resolved absolute]
    static final Object[][] FEATURES = {
        {"ore_dirt", 16, "uniform", -64, 320},          // count 16, uniform full? placeholder
        {null}
    };

    public static void main(String[] args) {
        long seed = Long.parseLong(args[0]);
        int cx = Integer.parseInt(args[1]);
        int cz = Integer.parseInt(args[2]);
        WorldgenRandom rng = new WorldgenRandom(new XoroshiroRandomSource(seed));
        long dec = rng.setDecorationSeed(seed, cx << 4, cz << 4);
        System.out.println("decorationSeed=" + dec);

        // Step-6 features in FeatureSorter order (from ProbeSorter6):
        // 0 ore_dirt, 1 ore_gravel, 2 ore_granite_upper, 3 ore_granite_lower,
        // 4 ore_diorite_upper, 5 ore_diorite_lower, 6 ore_andesite_upper,
        // 7 ore_andesite_lower, 8 ore_tuff, 9 ore_coal_upper, 10 ore_coal_lower,
        // 11 ore_iron_middle, 12 ore_iron_small, 13 ore_gold, 14 ore_gold_lower,
        // 15 ore_redstone, 16 ore_redstone_lower, 17 ore_diamond,
        // 18 ore_diamond_medium, 19 ore_diamond_large, 20 ore_diamond_buried,
        // 21 ore_lapis, 22 ore_lapis_buried, 23 ore_copper_large, 24 ore_copper,
        // 25 underwater_magma, 26 ore_clay, 27 ore_gold_extra, 28 disk_grass,
        // 29 disk_sand, 30 disk_clay, 31 disk_gravel, 32 ore_emerald
        //
        // NOTE: real indices include ALL step-6 features; we only model a few.
    }
}
