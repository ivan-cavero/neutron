import net.minecraft.SharedConstants;
import net.minecraft.core.HolderGetter;
import net.minecraft.core.registries.Registries;
import net.minecraft.data.registries.VanillaRegistries;
import net.minecraft.server.Bootstrap;
import net.minecraft.world.level.levelgen.DensityFunction;
import net.minecraft.world.level.levelgen.NoiseGeneratorSettings;
import net.minecraft.world.level.levelgen.RandomState;
import net.minecraft.world.level.levelgen.synth.NormalNoise;

/** Print vanilla aquifer fluid noises at explicit world cells. args: seed x y z [more...] */
public class ProbeFluidAtWorld {
    public static void main(String[] args) throws Exception {
        long seed = Long.parseLong(args[0]);
        SharedConstants.tryDetectVersion();
        Bootstrap.bootStrap();
        var lookup = VanillaRegistries.createLookup();
        HolderGetter<NormalNoise.NoiseParameters> noises = lookup.lookupOrThrow(Registries.NOISE);
        var settings = lookup.lookupOrThrow(Registries.NOISE_SETTINGS).getOrThrow(NoiseGeneratorSettings.OVERWORLD);
        RandomState rs = RandomState.create(settings.value(), noises, seed);
        var router = rs.router();
        for (int i = 1; i + 2 < args.length; i += 3) {
            int x = Integer.parseInt(args[i]);
            int y = Integer.parseInt(args[i + 1]);
            int z = Integer.parseInt(args[i + 2]);
            double f = router.fluidLevelFloodednessNoise().compute(new DensityFunction.SinglePointContext(x, y, z));
            double s = router.fluidLevelSpreadNoise().compute(new DensityFunction.SinglePointContext(x, y, z));
            double b = router.barrierNoise().compute(new DensityFunction.SinglePointContext(x, y, z));
            double ps = router.preliminarySurfaceLevel().compute(new DensityFunction.SinglePointContext(x, 0, z));
            System.out.println("(" + x + "," + y + "," + z + ") floodedness=" + String.format("%.6f", f)
                + " spread=" + String.format("%.6f", s) + " barrier=" + String.format("%.6f", b)
                + " prelim_surface=" + String.format("%.4f", ps));
        }
    }
}