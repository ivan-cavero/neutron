import java.util.ArrayList;
import java.util.List;
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

public class ProbeWalk {
    static List<DensityFunction> found = new ArrayList<>();
    static List<String> keys = new ArrayList<>();
    static void walk(DensityFunction f) {
        f.mapChildren(new DensityFunction.Visitor() {
            public DensityFunction apply(DensityFunction input) {
                if (input instanceof DensityFunctions.HolderHolder hh) {
                    var opt = hh.function().unwrapKey();
                    if (opt.isPresent()) {
                        String k = opt.get().identifier().toString();
                        if (k.contains("sloped_cheese") || k.contains("entrances") || k.contains("noodle") || k.contains("spaghetti_2d")) {
                            found.add(hh);
                            keys.add(k);
                        }
                    }
                }
                walk(input);
                return input;
            }
            public DensityFunction.NoiseHolder visitNoise(DensityFunction.NoiseHolder n) { return n; }
        });
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
        walk(r.finalDensity());
        int[][] coords = {{0,0,0}, {100,40,200}, {-57,63,31}, {511,100,-200}};
        for (int[] c : coords) {
            DensityFunction.SinglePointContext ctx = new DensityFunction.SinglePointContext(c[0], c[1], c[2]);
            System.out.printf("%s", String.format(Locale.ROOT, "-- (%d,%d,%d) --%n", c[0], c[1], c[2]));
            for (int i = 0; i < found.size(); i++) {
                double v = found.get(i).compute(ctx);
                System.out.printf("%s", String.format(Locale.ROOT, "%s=%.17g%n", keys.get(i), v));
            }
        }
    }
}
