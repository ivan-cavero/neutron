import java.util.Locale;
import net.minecraft.core.HolderGetter;
import net.minecraft.core.registries.Registries;
import net.minecraft.data.registries.VanillaRegistries;
import net.minecraft.world.level.levelgen.DensityFunction;
import net.minecraft.world.level.levelgen.NoiseGeneratorSettings;
import net.minecraft.world.level.levelgen.RandomState;
import net.minecraft.world.level.levelgen.synth.NormalNoise;

public class ProbeGaps {
    public static void main(String[] args) throws Exception {
        long seed = 12345L;
        net.minecraft.SharedConstants.tryDetectVersion();
        net.minecraft.server.Bootstrap.bootStrap();
        var lookup = VanillaRegistries.createLookup();
        HolderGetter<NormalNoise.NoiseParameters> noises = lookup.lookupOrThrow(Registries.NOISE);
        var settings = lookup.lookupOrThrow(Registries.NOISE_SETTINGS).getOrThrow(NoiseGeneratorSettings.OVERWORLD);
        RandomState rs = RandomState.create(settings.value(), noises, seed);
        DensityFunction fd = rs.router().finalDensity();
        int[][] pts = {
            {102,-41,-26},{96,-41,-24},{103,-40,-25},{103,-39,-28},
            {108,-38,-30},{105,-38,-29},{98,-38,-24},{107,-37,-30},
            {104,-37,-29},{99,-37,-25},{100,100,-20},{96,10,-20}
        };
        for (int[] p : pts) {
            var ctx = new DensityFunction.SinglePointContext(p[0],p[1],p[2]);
            double v = fd.compute(ctx);
            System.out.printf(Locale.ROOT, "%d,%d,%d  %.8f  %s%n", p[0],p[1],p[2], v, v>0?"solid":"air");
        }
    }
}
