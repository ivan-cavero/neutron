import java.util.Locale;
import net.minecraft.core.Holder;
import net.minecraft.core.HolderGetter;
import net.minecraft.core.registries.Registries;
import net.minecraft.data.registries.VanillaRegistries;
import net.minecraft.world.level.levelgen.DensityFunction;
import net.minecraft.world.level.levelgen.NoiseBasedChunkGenerator;
import net.minecraft.world.level.levelgen.NoiseGeneratorSettings;
import net.minecraft.world.level.levelgen.RandomState;
import net.minecraft.world.level.levelgen.synth.NormalNoise;

/**
 * Compare SinglePoint final_density vs real NoiseChunk-interpolated density
 * (NoiseBasedChunkGenerator.getInterpolatedNoiseValue).
 */
public class ProbeInterpDensity {
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
        DensityFunction fd = rs.router().finalDensity();

        int[][] pts = {
            {102, -41, -26}, {96, -41, -24}, {103, -40, -25}, {103, -39, -28},
            {108, -38, -30}, {98, -38, -24}, {96, -47, -20}, {96, -46, -23},
            {100, -36, -24}, {96, 64, -32}, {100, 40, -20}
        };
        System.out.println("wx,y,wz  singlePoint  noiseChunkInterp  baseSolid?");
        for (int[] p : pts) {
            var ctx = new DensityFunction.SinglePointContext(p[0], p[1], p[2]);
            double sp = fd.compute(ctx);
            double ni = gen.getInterpolatedNoiseValue(rs, ctx);
            System.out.printf(Locale.ROOT, "%d,%d,%d  %.8f  %.8f  sp=%s ni=%s%n",
                p[0], p[1], p[2], sp, ni,
                sp > 0 ? "solid" : "air",
                ni > 0 ? "solid" : "air");
        }
    }
}
