import net.minecraft.SharedConstants;
import net.minecraft.core.HolderGetter;
import net.minecraft.core.registries.Registries;
import net.minecraft.data.registries.VanillaRegistries;
import net.minecraft.server.Bootstrap;
import net.minecraft.world.level.levelgen.DensityFunction;
import net.minecraft.world.level.levelgen.NoiseGeneratorSettings;
import net.minecraft.world.level.levelgen.RandomState;
import net.minecraft.world.level.levelgen.synth.NormalNoise;

/** Print vanilla aquifer noises (floodedness/spread/barrier) at given cells. */
public class ProbeFluidAt {
    public static void main(String[] args) throws Exception {
        long seed = Long.parseLong(args[0]);
        SharedConstants.tryDetectVersion();
        Bootstrap.bootStrap();
        var lookup = VanillaRegistries.createLookup();
        HolderGetter<NormalNoise.NoiseParameters> noises = lookup.lookupOrThrow(Registries.NOISE);
        var settings = lookup.lookupOrThrow(Registries.NOISE_SETTINGS).getOrThrow(NoiseGeneratorSettings.OVERWORLD);
        RandomState rs = RandomState.create(settings.value(), noises, seed);
        var router = rs.router();
        int[][] pts = {{12,1,15},{10,2,15},{8,3,14},{2,5,14}};
        for (int[] p : pts) {
            double f = router.fluidLevelFloodednessNoise().compute(new DensityFunction.SinglePointContext(p[0], p[1], p[2]));
            double s = router.fluidLevelSpreadNoise().compute(new DensityFunction.SinglePointContext(p[0], p[1], p[2]));
            double b = router.barrierNoise().compute(new DensityFunction.SinglePointContext(p[0], p[1], p[2]));
            System.out.println("(" + p[0] + "," + p[1] + "," + p[2] + ") floodedness=" + String.format("%.4f", f)
                + " spread=" + String.format("%.4f", s) + " barrier=" + String.format("%.4f", b));
        }
    }
}