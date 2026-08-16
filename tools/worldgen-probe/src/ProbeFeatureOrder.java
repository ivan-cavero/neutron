import java.util.List;
import java.util.Set;
import net.minecraft.core.Holder;
import net.minecraft.core.registries.Registries;
import net.minecraft.data.registries.VanillaRegistries;
import net.minecraft.resources.ResourceKey;
import net.minecraft.world.level.biome.Biome;
import net.minecraft.world.level.biome.FeatureSorter;
import net.minecraft.world.level.biome.MultiNoiseBiomeSource;
import net.minecraft.world.level.biome.MultiNoiseBiomeSourceParameterLists;
import net.minecraft.world.level.levelgen.placement.PlacedFeature;

/**
 * Print overworld FeatureSorter global indices per generation step.
 * Vanilla ChunkGenerator.setFeatureSeed uses these indices, not biome-local ones.
 */
public class ProbeFeatureOrder {
    public static void main(String[] args) {
        net.minecraft.SharedConstants.tryDetectVersion();
        net.minecraft.server.Bootstrap.bootStrap();
        var lookup = VanillaRegistries.createLookup();
        var preset = lookup.lookupOrThrow(Registries.MULTI_NOISE_BIOME_SOURCE_PARAMETER_LIST)
            .getOrThrow(MultiNoiseBiomeSourceParameterLists.OVERWORLD);
        var biomeSource = MultiNoiseBiomeSource.createFromPreset(preset);

        System.out.println("=== possibleBiomes (iteration order) ===");
        int bi = 0;
        for (Holder<Biome> b : biomeSource.possibleBiomes()) {
            String name = b.unwrapKey().map(ResourceKey::identifier).map(Object::toString).orElse("?");
            System.out.printf("%3d  %s%n", bi++, name);
        }

        List<FeatureSorter.StepFeatureData> steps = FeatureSorter.buildFeaturesPerStep(
            List.copyOf(biomeSource.possibleBiomes()),
            holder -> holder.value().getGenerationSettings().features(),
            true
        );

        System.out.println("=== featuresPerStep ===");
        for (int step = 0; step < steps.size(); step++) {
            FeatureSorter.StepFeatureData data = steps.get(step);
            List<PlacedFeature> feats = data.features();
            System.out.printf("-- step %d (%d features) --%n", step, feats.size());
            for (int i = 0; i < feats.size(); i++) {
                PlacedFeature f = feats.get(i);
                String id = "?";
                // identity lookup via registry
                var reg = lookup.lookupOrThrow(Registries.PLACED_FEATURE);
                for (var e : reg.listElements().toList()) {
                    if (e.value() == f) {
                        id = e.key().identifier().toString();
                        break;
                    }
                }
                System.out.printf("  %3d  %s%n", i, id);
            }
        }
    }
}
