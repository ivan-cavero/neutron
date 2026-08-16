import java.lang.reflect.Field;
import java.util.ArrayList;
import java.util.List;
import java.util.Locale;
import net.minecraft.core.HolderGetter;
import net.minecraft.core.registries.Registries;
import net.minecraft.data.registries.VanillaRegistries;
import net.minecraft.world.level.levelgen.DensityFunction;
import net.minecraft.world.level.levelgen.NoiseGeneratorSettings;
import net.minecraft.world.level.levelgen.NoiseRouter;
import net.minecraft.world.level.levelgen.RandomState;
import net.minecraft.world.level.levelgen.synth.NormalNoise;

public class ProbeMin {
    static List<DensityFunction> dfs(Object o) throws Exception {
        List<DensityFunction> out = new ArrayList<>();
        for (Field f : o.getClass().getDeclaredFields()) {
            if (DensityFunction.class.isAssignableFrom(f.getType())) {
                f.setAccessible(true);
                out.add((DensityFunction) f.get(o));
            }
        }
        return out;
    }
    public static void main(String[] args) throws Exception {
        long seed = Long.parseLong(args[0]);
        net.minecraft.SharedConstants.tryDetectVersion();
        net.minecraft.server.Bootstrap.bootStrap();
        var lookup = VanillaRegistries.createLookup();
        HolderGetter<NormalNoise.NoiseParameters> noises = lookup.lookupOrThrow(Registries.NOISE);
        var settings = lookup.lookupOrThrow(Registries.NOISE_SETTINGS).getOrThrow(NoiseGeneratorSettings.OVERWORLD);
        RandomState rs = RandomState.create(settings.value(), noises, seed);
        NoiseRouter r = rs.router();
        DensityFunction finalDensity = r.finalDensity();
        List<DensityFunction> minArgs = dfs(finalDensity);
        DensityFunction arg1 = minArgs.get(0);  // postProcess
        DensityFunction arg2 = minArgs.get(1);  // noodle
        List<DensityFunction> sq = dfs(arg1);
        DensityFunction squeezeInner = sq.get(0);  // interpolated
        DensityFunction interp = dfs(squeezeInner).get(0);
        DensityFunction mul = dfs(interp).get(0);
        List<DensityFunction> mulArgs = dfs(mul);
        DensityFunction mulArg2 = mulArgs.get(0);  // blend_density (arg1 is the 0.64 constant)
        DensityFunction blendInner = dfs(mulArg2).get(0);  // slide
        int[][] coords = {{0,0,0}, {100,40,200}, {-57,63,31}, {511,100,-200}};
        for (int[] c : coords) {
            net.minecraft.world.level.levelgen.DensityFunction.SinglePointContext ctx = new net.minecraft.world.level.levelgen.DensityFunction.SinglePointContext(c[0], c[1], c[2]);
            double fd = finalDensity.compute(ctx);
            double a = arg1.compute(ctx);
            double n = arg2.compute(ctx);
            double interpV = interp.compute(ctx);
            double mulV = mul.compute(ctx);
            double slideV = blendInner.compute(ctx);
            System.out.printf("%s", String.format(Locale.ROOT, "(%d,%d,%d) final=%.17g A=%.17g noodle=%.17g interpolated=%.17g mul=%.17g slide=%.17g%n",
                c[0], c[1], c[2], fd, a, n, interpV, mulV, slideV));
        }
    }
}
