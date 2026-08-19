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
        long seed = Long.parseLong(args.length > 0 ? args[0] : "424242");
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
            {12, 1, 15}, {10, 2, 15}, {8, 3, 14}, {2, 5, 14}, {5, 5, 14}, {1, 5, 15}
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
