import java.util.Locale;
import net.minecraft.core.HolderGetter;
import net.minecraft.core.registries.Registries;
import net.minecraft.data.registries.VanillaRegistries;
import net.minecraft.world.level.levelgen.DensityFunction;
import net.minecraft.world.level.levelgen.NoiseGeneratorSettings;
import net.minecraft.world.level.levelgen.NoiseRouter;
import net.minecraft.world.level.levelgen.RandomState;
import net.minecraft.world.level.levelgen.synth.NormalNoise;

/** Dump router density function values for a seed at several coordinates. */
public class ProbeRouter {
    public static void main(String[] args) {
        long seed = Long.parseLong(args[0]);
        net.minecraft.SharedConstants.tryDetectVersion();
        net.minecraft.server.Bootstrap.bootStrap();
        var lookup = VanillaRegistries.createLookup();
        HolderGetter<NormalNoise.NoiseParameters> noises = lookup.lookupOrThrow(Registries.NOISE);
        var settings = lookup.lookupOrThrow(Registries.NOISE_SETTINGS).getOrThrow(NoiseGeneratorSettings.OVERWORLD);
        RandomState rs = RandomState.create(settings.value(), noises, seed);
        NoiseRouter r = rs.router();
        DensityFunction[] funcs = {
            r.barrierNoise(), r.fluidLevelFloodednessNoise(), r.fluidLevelSpreadNoise(), r.lavaNoise(),
            r.temperature(), r.vegetation(), r.continents(), r.erosion(), r.depth(), r.ridges(),
            r.preliminarySurfaceLevel(), r.finalDensity(), r.veinToggle(), r.veinRidged(), r.veinGap()
        };
        String[] names = {
            "barrier", "fluid_floodedness", "fluid_spread", "lava",
            "temperature", "vegetation", "continents", "erosion", "depth", "ridges",
            "preliminary_surface", "final_density", "vein_toggle", "vein_ridged", "vein_gap"
        };
        int[][] coords = {{0, 0, 0}, {100, 40, 200}, {-57, 63, 31}, {12, -40, 300}, {511, 100, -200}};
        for (int c = 0; c < coords.length; c++) {
            int x = coords[c][0], y = coords[c][1], z = coords[c][2];
            System.out.printf("%s", String.format(Locale.ROOT, "-- coord (%d,%d,%d) --%n", x, y, z));
            for (int i = 0; i < funcs.length; i++) {
                double v = funcs[i].compute(new DensityFunction.SinglePointContext(x, y, z));
                System.out.printf("%s", String.format(Locale.ROOT, "%s=%.17g%n", names[i], v));
            }
        }
    }
}
