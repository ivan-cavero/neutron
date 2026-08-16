import java.lang.reflect.Field;
import java.util.Locale;
import net.minecraft.core.HolderGetter;
import net.minecraft.core.registries.Registries;
import net.minecraft.data.registries.VanillaRegistries;
import net.minecraft.world.level.levelgen.DensityFunction;
import net.minecraft.world.level.levelgen.DensityFunctions;
import net.minecraft.world.level.levelgen.NoiseGeneratorSettings;
import net.minecraft.world.level.levelgen.NoiseRouter;
import net.minecraft.world.level.levelgen.RandomState;
import net.minecraft.world.level.levelgen.synth.NormalNoise;

public class ProbeSloped {
    static Object get(Object o, String n) throws Exception {
        if (o == null) return null;
        try {
            Field f = o.getClass().getDeclaredField(n);
            f.setAccessible(true);
            return f.get(o);
        } catch (NoSuchFieldException e) {
            for (Field f : o.getClass().getDeclaredFields()) {
                if (f.getName().toLowerCase().contains(n.toLowerCase()) || n.toLowerCase().contains(f.getName().toLowerCase())) {
                    f.setAccessible(true);
                    return f.get(o);
                }
            }
            return null;
        }
    }
    // find first node of the given class type in the tree
    static DensityFunction find(DensityFunction f, Class<?> cls) {
        if (cls.isInstance(f)) return f;
        final DensityFunction[] found = {null};
        f.mapChildren(new DensityFunction.Visitor() {
            public DensityFunction apply(DensityFunction input) {
                if (found[0] == null) found[0] = find(input, cls);
                return input;
            }
            public DensityFunction.NoiseHolder visitNoise(DensityFunction.NoiseHolder n) { return n; }
        });
        return found[0];
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
        int[][] coords = {{0,0,0}, {100,40,200}, {-57,63,31}, {12,-40,300}, {511,100,-200}};
        for (int[] c : coords) {
            DensityFunction.SinglePointContext ctx = new DensityFunction.SinglePointContext(c[0], c[1], c[2]);
            double fd = r.finalDensity().compute(ctx);
            double psl = r.preliminarySurfaceLevel().compute(ctx);
            System.out.printf("%s", String.format(Locale.ROOT, "(%d,%d,%d) final=%.17g prelim=%.17g%n", c[0], c[1], c[2], fd, psl));
        }
    }
}
