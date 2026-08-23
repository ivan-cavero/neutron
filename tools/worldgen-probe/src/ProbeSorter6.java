import java.util.List;
import net.minecraft.SharedConstants;
import net.minecraft.core.Holder;
import net.minecraft.core.registries.Registries;
import net.minecraft.server.Bootstrap;
import net.minecraft.world.level.biome.Biome;
import net.minecraft.world.level.biome.FeatureSorter;
import net.minecraft.world.level.levelgen.placement.PlacedFeature;

/** Print the REAL FeatureSorter step-6 indices for ore/disk features. */
public class ProbeSorter6 {
    public static void main(String[] args) throws Exception {
        SharedConstants.tryDetectVersion();
        Bootstrap.bootStrap();
        var lookup = net.minecraft.data.registries.VanillaRegistries.createLookup();
        var biomes = lookup.lookupOrThrow(Registries.BIOME);
        var placed = lookup.lookupOrThrow(Registries.PLACED_FEATURE);
        var paramList = lookup.lookupOrThrow(Registries.MULTI_NOISE_BIOME_SOURCE_PARAMETER_LIST)
                .getOrThrow(net.minecraft.resources.ResourceKey.create(
                        Registries.MULTI_NOISE_BIOME_SOURCE_PARAMETER_LIST,
                        net.minecraft.resources.Identifier.parse("minecraft:overworld")));
        var possible = paramList.value().parameters().values().stream()
                .map(com.mojang.datafixers.util.Pair::getSecond).distinct().toList();
        List<Holder<Biome>> all = new java.util.ArrayList<>(possible);
        System.out.println("possibleBiomes count=" + all.size());
        var sorter = FeatureSorter.buildFeaturesPerStep(all, b -> b.value().getGenerationSettings().features(), true);
        System.out.println("=== step 6 full index list ===");
        var step6 = sorter.get(6).features();
        for (int i = 0; i < step6.size(); i++) {
            // PlacedFeature has no name at runtime here; print by scanning registry order
            String name = "?";
            for (var e : placed.listElements().toList()) {
                if (e.value() == step6.get(i)) { name = e.key().identifier().toString(); break; }
            }
            System.out.println(i + " " + name);
        }
    }
}
