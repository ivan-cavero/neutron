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

public class ProbeInner {
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
    static void findRangeChoice(DensityFunction f, List<DensityFunction> out) {
        if (f.getClass().getSimpleName().contains("RangeChoice")) { out.add(f); return; }
        try {
            for (DensityFunction k : dfs(f)) {
                if (out.isEmpty()) findRangeChoice(k, out);
            }
        } catch (Exception e) {}
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
        List<DensityFunction> rcs = new ArrayList<>();
        findRangeChoice(r.finalDensity(), rcs);
        DensityFunction cavesRC = rcs.get(0);
        List<DensityFunction> rcKids = dfs(cavesRC);
        DensityFunction input = rcKids.get(0);          // Marker(cacheOnce)
        DensityFunction holderHolder = dfs(input).get(0); // HolderHolder
        Field hf = holderHolder.getClass().getDeclaredField("function");
        hf.setAccessible(true);
        DensityFunction slopedCheese = ((net.minecraft.core.Holder<DensityFunction>) hf.get(holderHolder)).value(); // add(initialDensity, base_3d)
        List<DensityFunction> scArgs = dfs(slopedCheese);
        DensityFunction initialDensity = scArgs.get(0);  // mul(4, qn)
        DensityFunction qn = dfs(initialDensity).get(0); // Mapped quarterNegative
        DensityFunction qnInner = dfs(qn).get(0);        // mul(add(depth,jagged), factor)
        List<DensityFunction> qnArgs = dfs(qnInner);
        DensityFunction depthJagged = qnArgs.get(0);
        DensityFunction factor = qnArgs.get(1);
        DensityFunction jaggedChain = dfs(depthJagged).get(1);  // flatCache(mul(jaggedness, halfneg))
        DensityFunction jaggedMul = dfs(jaggedChain).get(0);
        DensityFunction jaggednessSpline = dfs(jaggedMul).get(0);
        DensityFunction jaggedNoiseHalf = dfs(jaggedMul).get(1);
        int[][] coords = {{100,40,200}, {-57,63,31}, {511,100,-200}, {0,0,0}};
        for (int[] c : coords) {
            net.minecraft.world.level.levelgen.DensityFunction.SinglePointContext ctx = new net.minecraft.world.level.levelgen.DensityFunction.SinglePointContext(c[0], c[1], c[2]);
            System.out.printf("%s", String.format(Locale.ROOT, "(%d,%d,%d) sloped=%.17g initialDensity=%.17g qnInner=%.17g factor=%.17g jaggedness=%.17g jaggedHalf=%.17g%n",
                c[0], c[1], c[2], slopedCheese.compute(ctx), initialDensity.compute(ctx), qnInner.compute(ctx), factor.compute(ctx), jaggednessSpline.compute(ctx), jaggedNoiseHalf.compute(ctx)));
        }
    }
}
