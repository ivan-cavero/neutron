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
 * Sample vanilla base column (density+aquifer+ore veins, NO carvers/features)
 * at pure-air gap coords for chunk (6,-2) seed 12345.
 */
public class ProbeBaseColumn {
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

        // Biome source not used by iterateNoiseColumn material rules beyond aquifer;
        // fixed plains is enough for base density+aquifer sampling.
        HolderGetter<net.minecraft.world.level.biome.Biome> biomes =
            lookup.lookupOrThrow(Registries.BIOME);
        var plains = biomes.getOrThrow(net.minecraft.world.level.biome.Biomes.PLAINS);
        var biomeSource = new net.minecraft.world.level.biome.FixedBiomeSource(plains);

        NoiseBasedChunkGenerator gen = new NoiseBasedChunkGenerator(biomeSource, settings);
        RandomState rs = RandomState.create(settings.value(), noises, seed);

        // pure-air gap world coords from air_gaps example + sculk samples
        int[][] pts = {
            {102, -41, -26}, {96, -41, -24}, {103, -40, -25}, {103, -39, -28},
            {108, -38, -30}, {98, -38, -24}, {96, -47, -20}, {96, -46, -23},
            {98, -44, -25}, {100, -36, -24}, {104, -30, -24}, {100, 0, -20},
            {100, 40, -20}, {96, 64, -32}
        };

        System.out.println("wx,y,wz  baseBlock  isAirOrFluid  (from getBaseColumn — no carvers)");
        int air = 0, solid = 0;
        for (int[] p : pts) {
            int x = p[0], y = p[1], z = p[2];
            NoiseColumn col = gen.getBaseColumn(x, z, HEIGHT, rs);
            BlockState st = col.getBlock(y);
            boolean open = st.isAir() || !st.getFluidState().isEmpty();
            if (open) air++; else solid++;
            System.out.printf(Locale.ROOT, "%d,%d,%d  %s  %s%n",
                x, y, z, st.getBlock(), open ? "OPEN" : "SOLID");
        }
        System.out.printf(Locale.ROOT, "summary open=%d solid=%d%n", air, solid);

        // Full deep column scan at local (6,6) → world (102,-26): count open Y in [-64,64)
        int wx = 102, wz = -26;
        NoiseColumn col = gen.getBaseColumn(wx, wz, HEIGHT, rs);
        int openCount = 0;
        System.out.println("--- column (102,z=-26) open Y in [-64,64) ---");
        for (int y = -64; y < 64; y++) {
            BlockState st = col.getBlock(y);
            boolean open = st.isAir() || !st.getFluidState().isEmpty();
            if (open) {
                openCount++;
                if (openCount <= 30) {
                    System.out.printf(Locale.ROOT, "  y=%d %s%n", y, st.getBlock());
                }
            }
        }
        System.out.println("column open count [-64,64) = " + openCount);

        // Compare SinglePoint final_density at first gap
        var fd = rs.router().finalDensity();
        for (int[] p : new int[][]{{102,-41,-26},{96,-47,-20}}) {
            var ctx = new net.minecraft.world.level.levelgen.DensityFunction.SinglePointContext(p[0], p[1], p[2]);
            double v = fd.compute(ctx);
            System.out.printf(Locale.ROOT, "SinglePoint final(%d,%d,%d)=%.6f solid?%s%n",
                p[0], p[1], p[2], v, v > 0 ? "yes" : "no");
        }
    }
}
