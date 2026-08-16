import java.util.Locale;
import net.minecraft.core.Holder;
import net.minecraft.core.HolderGetter;
import net.minecraft.core.registries.Registries;
import net.minecraft.data.registries.VanillaRegistries;
import net.minecraft.world.level.LevelHeightAccessor;
import net.minecraft.world.level.NoiseColumn;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.levelgen.NoiseBasedChunkGenerator;
import net.minecraft.world.level.levelgen.NoiseGeneratorSettings;
import net.minecraft.world.level.levelgen.RandomState;
import net.minecraft.world.level.levelgen.synth.NormalNoise;

/**
 * Multi-chunk base-column occupancy (density+aquifer, no features/carvers)
 * for several world positions — multi-biome density ground truth without MCA.
 */
public class ProbeMultiBase {
    static final LevelHeightAccessor HEIGHT = new LevelHeightAccessor() {
        @Override public int getHeight() { return 384; }
        @Override public int getMinY() { return -64; }
    };

    public static void main(String[] args) throws Exception {
        long seed = 12345L;
        net.minecraft.SharedConstants.tryDetectVersion();
        net.minecraft.server.Bootstrap.bootStrap();
        var lookup = VanillaRegistries.createLookup();
        HolderGetter<NormalNoise.NoiseParameters> noises = lookup.lookupOrThrow(Registries.NOISE);
        Holder<NoiseGeneratorSettings> settings =
            lookup.lookupOrThrow(Registries.NOISE_SETTINGS).getOrThrow(NoiseGeneratorSettings.OVERWORLD);
        var biomes = lookup.lookupOrThrow(Registries.BIOME);
        var plains = biomes.getOrThrow(net.minecraft.world.level.biome.Biomes.PLAINS);
        var biomeSource = new net.minecraft.world.level.biome.FixedBiomeSource(plains);
        NoiseBasedChunkGenerator gen = new NoiseBasedChunkGenerator(biomeSource, settings);
        RandomState rs = RandomState.create(settings.value(), noises, seed);

        // Chunk centers (world block sample at +8,+8)
        int[][] chunks = {
            {0, 0}, {6, -2}, {32, 0}, {-32, 16}, {0, 48}, {64, -32}, {-48, -48},
            {10, 10}, {20, -5}, {5, -3}, {100, 0}, {-100, 50}
        };
        System.out.println("cx,cz  open_frac  solid_frac  (base column sample 16 columns)");
        for (int[] c : chunks) {
            int cx = c[0], cz = c[1];
            int open = 0, solid = 0, total = 0;
            for (int lx = 0; lx < 16; lx += 4) {
                for (int lz = 0; lz < 16; lz += 4) {
                    int wx = cx * 16 + lx;
                    int wz = cz * 16 + lz;
                    NoiseColumn col = gen.getBaseColumn(wx, wz, HEIGHT, rs);
                    for (int y = -64; y < 320; y++) {
                        BlockState st = col.getBlock(y);
                        total++;
                        if (st.isAir() || !st.getFluidState().isEmpty()) open++;
                        else solid++;
                    }
                }
            }
            System.out.printf(Locale.ROOT, "%d,%d  open=%.4f solid=%.4f  n=%d%n",
                cx, cz, open / (double) total, solid / (double) total, total);
        }
    }
}
