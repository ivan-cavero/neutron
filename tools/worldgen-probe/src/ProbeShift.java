import java.util.Locale;
import net.minecraft.core.HolderGetter;
import net.minecraft.core.registries.Registries;
import net.minecraft.data.registries.VanillaRegistries;
import net.minecraft.world.level.levelgen.DensityFunction;
import net.minecraft.world.level.levelgen.NoiseGeneratorSettings;
import net.minecraft.world.level.levelgen.NoiseRouter;
import net.minecraft.world.level.levelgen.RandomState;
import net.minecraft.world.level.levelgen.synth.NormalNoise;

public class ProbeShift {
    static Object get(java.lang.reflect.Field f, Object o) throws Exception { f.setAccessible(true); return f.get(o); }
    public static void main(String[] args) throws Exception {
        long seed = Long.parseLong(args[0]);
        net.minecraft.SharedConstants.tryDetectVersion();
        net.minecraft.server.Bootstrap.bootStrap();
        var lookup = VanillaRegistries.createLookup();
        HolderGetter<NormalNoise.NoiseParameters> noises = lookup.lookupOrThrow(Registries.NOISE);
        var settings = lookup.lookupOrThrow(Registries.NOISE_SETTINGS).getOrThrow(NoiseGeneratorSettings.OVERWORLD);
        RandomState rs = RandomState.create(settings.value(), noises, seed);
        NoiseRouter r = rs.router();
        // temperature field is ShiftedNoise; pull shift_x / shift_z children
        int[][] coords = {{100, 40, 200}, {-57, 63, 31}, {12, -40, 300}, {511, 100, -200}, {0, 0, 0}};
        for (int[] c : coords) {
            DensityFunction.SinglePointContext ctx = new DensityFunction.SinglePointContext(c[0], c[1], c[2]);
            double t = r.temperature().compute(ctx);
            double cval = r.continents().compute(ctx);
            double ridge = r.ridges().compute(ctx);
            // also compute the shift functions directly via temperature's mapChildren
            DensityFunction temp2 = r.temperature().mapChildren(f -> f);
            java.lang.reflect.Field f = temp2.getClass().getDeclaredField("shiftX");
            java.lang.reflect.Field f2 = temp2.getClass().getDeclaredField("shiftZ");
            double sx = ((DensityFunction) get(f, temp2)).compute(ctx);
            double sz = ((DensityFunction) get(f2, temp2)).compute(ctx);
            System.out.printf("%s", String.format(Locale.ROOT, "(%d,%d,%d) temp=%.17g continents=%.17g ridges=%.17g shiftX=%.17g shiftZ=%.17g%n",
                c[0], c[1], c[2], t, cval, ridge, sx, sz));
        }
    }
}
