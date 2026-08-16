import java.util.Locale;
import net.minecraft.core.HolderGetter;
import net.minecraft.core.registries.Registries;
import net.minecraft.data.registries.VanillaRegistries;
import net.minecraft.world.level.levelgen.DensityFunction;
import net.minecraft.world.level.levelgen.NoiseGeneratorSettings;
import net.minecraft.world.level.levelgen.RandomState;
import net.minecraft.world.level.levelgen.synth.NormalNoise;
import net.minecraft.resources.ResourceKey;
import net.minecraft.world.level.levelgen.DensityFunctions;

public class ProbeFuncs {
    public static void main(String[] args) throws Exception {
        long seed = Long.parseLong(args[0]);
        net.minecraft.SharedConstants.tryDetectVersion();
        net.minecraft.server.Bootstrap.bootStrap();
        var lookup = VanillaRegistries.createLookup();
        HolderGetter<NormalNoise.NoiseParameters> noises = lookup.lookupOrThrow(Registries.NOISE);
        var settings = lookup.lookupOrThrow(Registries.NOISE_SETTINGS).getOrThrow(NoiseGeneratorSettings.OVERWORLD);
        RandomState rs = RandomState.create(settings.value(), noises, seed);
        var funcs = lookup.lookupOrThrow(Registries.DENSITY_FUNCTION);
        String[] keys = {"overworld/offset", "overworld/factor", "overworld/jaggedness", "overworld/sloped_cheese", "overworld/depth", "overworld/caves/entrances", "overworld/caves/noodle", "overworld/caves/spaghetti_2d"};
        int[][] coords = {{0,0,0}, {100,40,200}, {-57,63,31}, {12,-40,300}};
        for (int c = 0; c < coords.length; c++) {
            int x = coords[c][0], y = coords[c][1], z = coords[c][2];
            DensityFunction.SinglePointContext ctx = new DensityFunction.SinglePointContext(x, y, z);
            System.out.printf("%s", String.format(Locale.ROOT, "-- (%d,%d,%d) --%n", x, y, z));
            for (String k : keys) {
                var h = funcs.getOrThrow(ResourceKey.create(Registries.DENSITY_FUNCTION, net.minecraft.resources.Identifier.withDefaultNamespace(k)));
                double v = h.value().compute(ctx);
                System.out.printf("%s", String.format(Locale.ROOT, "%s=%.17g%n", k, v));
            }
        }
    }
}
