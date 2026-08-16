import java.util.ArrayList;
import java.util.HashSet;
import java.util.IdentityHashMap;
import java.util.List;
import java.util.Map;
import java.util.Set;

import net.minecraft.core.Holder;
import net.minecraft.core.HolderGetter;
import net.minecraft.core.registries.Registries;
import net.minecraft.data.registries.VanillaRegistries;
import net.minecraft.resources.Identifier;
import net.minecraft.resources.ResourceKey;
import net.minecraft.world.level.biome.Climate;
import net.minecraft.world.level.levelgen.DensityFunction;
import net.minecraft.world.level.levelgen.DensityFunctions;
import net.minecraft.world.level.levelgen.NoiseGeneratorSettings;
import net.minecraft.world.level.levelgen.NoiseRouter;
import net.minecraft.world.level.levelgen.synth.BlendedNoise;
import net.minecraft.world.level.levelgen.synth.NormalNoise;

/**
 * Empirically derive the noise instantiation ORDER during RandomState construction.
 *
 * Replicates RandomState's NoiseWiringHelper traversal: mapAll over the router
 * visits every node; visitNoise(NoiseHolder) triggers noise creation the first
 * time a key is seen (computeIfAbsent semantics).
 */
public class ProbeOrder {
    static int counter = 0;

    static void walk(DensityFunction f, Set<String> created, Map<DensityFunction, Object> memo) {
        if (memo.containsKey(f)) return;
        // mapAll RecursiveVisitor: apply(f) = visitor.apply(f.mapChildren(this))
        f.mapChildren(new DensityFunction.Visitor() {
            @Override
            public DensityFunction apply(DensityFunction input) {
                walk(input, created, memo);
                return input;
            }
            @Override
            public DensityFunction.NoiseHolder visitNoise(DensityFunction.NoiseHolder noise) {
                var opt = noise.noiseData().unwrapKey();
                if (opt.isPresent()) {
                    String key = opt.get().identifier().toString();
                    if (created.add(key)) {
                        System.out.printf("%2d  %s%n", counter++, key);
                    }
                }
                return noise;
            }
        });
        memo.put(f, Boolean.TRUE);
    }

    static void walkRouter(NoiseRouter router) {
        DensityFunction[] fields = new DensityFunction[] {
            router.barrierNoise(), router.fluidLevelFloodednessNoise(),
            router.fluidLevelSpreadNoise(), router.lavaNoise(),
            router.temperature(), router.vegetation(),
            router.continents(), router.erosion(), router.depth(), router.ridges(),
            router.preliminarySurfaceLevel(), router.finalDensity(),
            router.veinToggle(), router.veinRidged(), router.veinGap()
        };
        Set<String> created = new HashSet<>();
        Map<DensityFunction, Object> memo = new IdentityHashMap<>();
        for (int i = 0; i < fields.length; i++) {
            System.out.printf("-- field %d --%n", i);
            walk(fields[i], created, memo);
        }
    }

    public static void main(String[] args) {
        net.minecraft.SharedConstants.tryDetectVersion();
        net.minecraft.server.Bootstrap.bootStrap();
        var lookup = VanillaRegistries.createLookup();
        HolderGetter<NormalNoise.NoiseParameters> noises = lookup.lookupOrThrow(Registries.NOISE);
        Holder<NoiseGeneratorSettings> settings = lookup.lookupOrThrow(Registries.NOISE_SETTINGS)
            .getOrThrow(NoiseGeneratorSettings.OVERWORLD);
        NoiseRouter router = settings.value().noiseRouter();
        System.out.println("=== noise instantiation order (overworld router) ===");
        walkRouter(router);
    }
}
