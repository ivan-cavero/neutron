import java.util.IdentityHashMap;
import java.util.List;
import java.util.Map;
import java.util.Locale;
import net.minecraft.SharedConstants;
import net.minecraft.core.Holder;
import net.minecraft.core.registries.Registries;
import net.minecraft.data.registries.VanillaRegistries;
import net.minecraft.resources.ResourceKey;
import net.minecraft.server.Bootstrap;
import net.minecraft.world.level.biome.Biome;
import net.minecraft.world.level.biome.FeatureSorter;
import net.minecraft.world.level.biome.MultiNoiseBiomeSource;
import net.minecraft.world.level.biome.MultiNoiseBiomeSourceParameterLists;
import net.minecraft.world.level.levelgen.WorldgenRandom;
import net.minecraft.world.level.levelgen.placement.PlacedFeature;

/**
 * DEFINITIVE per-step global FeatureSorter index table for the overworld,
 * vanilla-side. Mirrors the real pipeline exactly:
 *   - ChunkGenerator.java:96-98 builds featuresPerStep ONCE from
 *     List.copyOf(biomeSource.possibleBiomes()) via FeatureSorter
 *     .buildFeaturesPerStep (FeatureSorter.java:28-110).
 *   - ChunkGenerator.applyBiomeDecoration (:389) then re-seeds each feature
 *     with random.setFeatureSeed(decorationSeed, globalIndex, step), where
 *     globalIndex is the position in that per-step list. The per-chunk
 *     possibleBiomes.retainAll (:336) gates PLACEMENT only, not indices.
 *
 * Also prints live RNG draws per index for decorationSeed =
 * setDecorationSeed(424242, -16, -32) [origin chunk (-1,-2)] so the Rust side
 * can diff byte streams if indices drift.
 *
 * Output lines (stable, diffable):
 *   BIOME <i> <id>                      possibleBiomes iteration order
 *   IDX <step> <index> <id>             global index table
 *   DEC <decorationSeed>                seed for origin (-1,-2), level seed 424242
 *   DRAW <step> <index> <id> d1..d6 fb  nextInt(16)x2, nextInt(256), nextInt(16)x3, nextFloat raw bits
 */
public class ProbeIndexTable {
    public static void main(String[] args) throws Exception {
        SharedConstants.tryDetectVersion();
        Bootstrap.bootStrap();
        var lookup = VanillaRegistries.createLookup();

        // Overworld biome source = the real launcher's biome set + order.
        var preset = lookup.lookupOrThrow(Registries.MULTI_NOISE_BIOME_SOURCE_PARAMETER_LIST)
            .getOrThrow(MultiNoiseBiomeSourceParameterLists.OVERWORLD);
        var biomeSource = MultiNoiseBiomeSource.createFromPreset(preset);
        List<Holder<Biome>> biomes = List.copyOf(biomeSource.possibleBiomes());

        System.out.println("=== possibleBiomes (iteration order) ===");
        int bi = 0;
        for (Holder<Biome> b : biomes) {
            System.out.printf(Locale.ROOT, "BIOME %d %s%n", bi++, id(b));
        }

        List<FeatureSorter.StepFeatureData> steps = FeatureSorter.buildFeaturesPerStep(
            biomes, holder -> holder.value().getGenerationSettings().features(), true);

        // Identity map PlacedFeature -> registry id (built once).
        Map<PlacedFeature, String> names = new IdentityHashMap<>();
        for (var e : lookup.lookupOrThrow(Registries.PLACED_FEATURE).listElements().toList()) {
            names.put(e.value(), e.key().identifier().toString());
        }

        System.out.println("=== featuresPerStep ===");
        for (int step = 0; step < steps.size(); step++) {
            List<PlacedFeature> feats = steps.get(step).features();
            System.out.printf(Locale.ROOT, "STEP %d %d%n", step, feats.size());
            for (int i = 0; i < feats.size(); i++) {
                System.out.printf(Locale.ROOT, "IDX %d %d %s%n", step, i, names.getOrDefault(feats.get(i), "?"));
            }
        }

        // Live draws: decoration seed for level seed 424242, origin block (-16, -32).
        WorldgenRandom rng = new WorldgenRandom(new net.minecraft.world.level.levelgen.XoroshiroRandomSource(0L));
        long decorationSeed = rng.setDecorationSeed(424242L, -16, -32);
        System.out.printf(Locale.ROOT, "DEC %d%n", decorationSeed);

        drawFor(rng, decorationSeed, steps, names, 9, 20, 35);
        drawFor(rng, decorationSeed, steps, names, 6, 20, 30);
    }

    static void drawFor(WorldgenRandom proto, long decorationSeed,
                        List<FeatureSorter.StepFeatureData> steps,
                        Map<PlacedFeature, String> names, int step, int lo, int hi) {
        List<PlacedFeature> feats = steps.get(step).features();
        for (int i = lo; i <= hi && i < feats.size(); i++) {
            WorldgenRandom rng = new WorldgenRandom(new net.minecraft.world.level.levelgen.XoroshiroRandomSource(0L));
            rng.setFeatureSeed(decorationSeed, i, step);
            int d1 = rng.nextInt(16), d2 = rng.nextInt(16);       // in_square x/z
            int d3 = rng.nextInt(256);                            // generic height/offset sample
            int d4 = rng.nextInt(16), d5 = rng.nextInt(16), d6 = rng.nextInt(16);
            float fb = rng.nextFloat();
            System.out.printf(Locale.ROOT, "DRAW %d %d %s %d %d %d %d %d %d %08x%n",
                step, i, names.getOrDefault(feats.get(i), "?"), d1, d2, d3, d4, d5, d6,
                Float.floatToRawIntBits(fb));
        }
    }

    static String id(Holder<Biome> b) {
        return b.unwrapKey().map(k -> k.identifier().toString()).orElse("?");
    }
}
