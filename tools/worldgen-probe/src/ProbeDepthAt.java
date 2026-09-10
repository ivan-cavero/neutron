import net.minecraft.SharedConstants;
import net.minecraft.core.HolderGetter;
import net.minecraft.core.registries.Registries;
import net.minecraft.data.registries.VanillaRegistries;
import net.minecraft.server.Bootstrap;
import net.minecraft.world.level.levelgen.DensityFunction;
import net.minecraft.world.level.levelgen.NoiseGeneratorSettings;
import net.minecraft.world.level.levelgen.RandomState;
import net.minecraft.world.level.levelgen.synth.NormalNoise;
import net.minecraft.core.HolderGetter;

public class ProbeDepthAt {
    public static void main(String[] args) throws Exception {
        long seed = Long.parseLong(args[0]);
        int wx = Integer.parseInt(args[1]);
        int y0 = Integer.parseInt(args[2]);
        int y1 = Integer.parseInt(args[3]);
        int wz = Integer.parseInt(args[4]);
        
        SharedConstants.tryDetectVersion();
        Bootstrap.bootStrap();
        var lookup = VanillaRegistries.createLookup();
        HolderGetter<NormalNoise.NoiseParameters> noises = lookup.lookupOrThrow(Registries.NOISE);
        var settings = lookup.lookupOrThrow(Registries.NOISE_SETTINGS).getOrThrow(NoiseGeneratorSettings.OVERWORLD);
        RandomState rs = RandomState.create(settings.value(), noises, seed);
        var router = rs.router();
        
        System.out.println("y     depth       ridges      erosion     surf_level");
        for (int y = y0; y <= y1; y++) {
            DensityFunction.SinglePointContext ctx = new DensityFunction.SinglePointContext(wx, y, wz);
            double depth = router.depth().compute(ctx);
            double ridges = router.ridges().compute(ctx);
            double erosion = router.erosion().compute(ctx);
            double surf = router.preliminarySurfaceLevel().compute(ctx);
            System.out.printf("%4d  %10.6f  %10.6f  %10.6f  %10.6f%n", y, depth, ridges, erosion, surf);
        }
    }
}
