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

public class ProbeLevels {
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
        DensityFunction arg1 = dfs(finalDensity).get(0);       // Mapped (squeeze)
        DensityFunction marker1 = dfs(arg1).get(0);            // Marker interpolated
        DensityFunction mulOrAdd = dfs(marker1).get(0);        // MulOrAdd 0.64*x
        DensityFunction marker2 = dfs(mulOrAdd).get(0);        // Marker blend_density
        DensityFunction inner2 = dfs(marker2).get(0);          // MulOrAdd (bottomFactor chain)
        int[][] coords = {{100,40,200}, {-57,63,31}, {511,100,-200}};
        for (int[] c : coords) {
            net.minecraft.world.level.levelgen.DensityFunction.SinglePointContext ctx = new net.minecraft.world.level.levelgen.DensityFunction.SinglePointContext(c[0], c[1], c[2]);
            System.out.printf("%s", String.format(Locale.ROOT, "(%d,%d,%d) A=%.17g interp=%.17g mul64=%.17g blend=%.17g slide=%.17g noodle=%.17g%n",
                c[0], c[1], c[2], arg1.compute(ctx), marker1.compute(ctx), mulOrAdd.compute(ctx), marker2.compute(ctx), inner2.compute(ctx), dfs(finalDensity).get(1).compute(ctx)));
        }
    }
}
