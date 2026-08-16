import java.util.Locale;
import java.util.List;
import net.minecraft.core.HolderGetter;
import net.minecraft.core.registries.Registries;
import net.minecraft.data.registries.VanillaRegistries;
import net.minecraft.world.level.levelgen.DensityFunction;
import net.minecraft.world.level.levelgen.DensityFunctions;
import net.minecraft.world.level.levelgen.NoiseGeneratorSettings;
import net.minecraft.world.level.levelgen.NoiseRouter;
import net.minecraft.world.level.levelgen.RandomState;
import net.minecraft.world.level.levelgen.synth.NormalNoise;

/** Compare final_density parts at cave-wall coords for seed 12345. */
public class ProbeNoodle {
    static List<DensityFunction> children(DensityFunction f) {
        // Reflective walk of TwoArgumentSimpleFunction etc is hard; use known structure:
        // final = min(squeeze(interp(...)), noodle)
        return null;
    }

    public static void main(String[] args) throws Exception {
        long seed = 12345L;
        net.minecraft.SharedConstants.tryDetectVersion();
        net.minecraft.server.Bootstrap.bootStrap();
        var lookup = VanillaRegistries.createLookup();
        HolderGetter<NormalNoise.NoiseParameters> noises = lookup.lookupOrThrow(Registries.NOISE);
        var settings = lookup.lookupOrThrow(Registries.NOISE_SETTINGS).getOrThrow(NoiseGeneratorSettings.OVERWORLD);
        RandomState rs = RandomState.create(settings.value(), noises, seed);
        DensityFunction fd = rs.router().finalDensity();
        // Unwrap min
        DensityFunction a = null, b = null;
        if (fd instanceof DensityFunctions.TwoArgumentSimpleFunction two) {
            // type min?
            a = two.argument1();
            b = two.argument2();
        } else {
            // try record accessors via reflection
            var m1 = fd.getClass().getMethod("argument1");
            var m2 = fd.getClass().getMethod("argument2");
            a = (DensityFunction) m1.invoke(fd);
            b = (DensityFunction) m2.invoke(fd);
        }
        int[][] pts = {
            {96,-47,-20},{96,-46,-23},{97,-46,-21},{98,-45,-22},
            {102,-41,-26},{96,-41,-24},{103,-40,-25},{106,-39,-29},
            {100,0,-20},{100,200,-20}
        };
        System.out.println("wx,y,wz  arg1(squeezed path)  arg2(noodle)  final  solid?");
        for (int[] p : pts) {
            int x=p[0],y=p[1],z=p[2];
            var ctx = new DensityFunction.SinglePointContext(x,y,z);
            double va = a.compute(ctx);
            double vb = b.compute(ctx);
            double vf = fd.compute(ctx);
            System.out.printf(Locale.ROOT, "%d,%d,%d  %.8f  %.8f  %.8f  %s%n",
                x,y,z, va, vb, vf, vf>0?"solid":"air");
        }
    }
}
