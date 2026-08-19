import net.minecraft.SharedConstants;
import net.minecraft.core.HolderGetter;
import net.minecraft.core.registries.Registries;
import net.minecraft.data.registries.VanillaRegistries;
import net.minecraft.server.Bootstrap;
import net.minecraft.world.level.levelgen.DensityFunction;
import net.minecraft.world.level.levelgen.NoiseGeneratorSettings;
import net.minecraft.world.level.levelgen.RandomState;
import net.minecraft.world.level.levelgen.synth.NormalNoise;

/** Print the vanilla finalDensity (and the aquifer barrier) at the missing-water cells. */
public class ProbeFinalDensity {
    public static void main(String[] args) throws Exception {
        long seed = Long.parseLong(args[0]);
        SharedConstants.tryDetectVersion();
        Bootstrap.bootStrap();
        var lookup = VanillaRegistries.createLookup();
        HolderGetter<NormalNoise.NoiseParameters> noises = lookup.lookupOrThrow(Registries.NOISE);
        var settings = lookup.lookupOrThrow(Registries.NOISE_SETTINGS).getOrThrow(NoiseGeneratorSettings.OVERWORLD);
        RandomState rs = RandomState.create(settings.value(), noises, seed);
        var router = rs.router();
        int[][] pts = {{12,0,12},{12,0,16},{12,8,12},{12,8,16},{16,0,12},{16,0,16},{16,8,12},{16,8,16},{12,1,15}};
        for (int[] p : pts) {
            DensityFunction.SinglePointContext ctx = new DensityFunction.SinglePointContext(p[0], p[1], p[2]);
            double f = router.finalDensity().compute(ctx);
            double b = router.barrierNoise().compute(ctx);
            System.out.println("(" + p[0] + "," + p[1] + "," + p[2] + ") final=" + String.format("%.6f", f)
                + " barrier=" + String.format("%.4f", b));
        }
    }
}