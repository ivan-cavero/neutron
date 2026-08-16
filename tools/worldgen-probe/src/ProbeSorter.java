import java.util.List;
import net.minecraft.SharedConstants;
import net.minecraft.core.Holder;
import net.minecraft.core.registries.Registries;
import net.minecraft.server.Bootstrap;
import net.minecraft.world.level.biome.Biome;
import net.minecraft.world.level.biome.FeatureSorter;
import net.minecraft.world.level.levelgen.placement.PlacedFeature;

/** Print the REAL FeatureSorter step-9 indices for pale garden features. */
public class ProbeSorter {
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
        System.out.println("=== step 9 index list ===");
        var step9 = sorter.get(9).features();
        var want = new String[] {
            "minecraft:pale_garden_vegetation", "minecraft:glow_lichen",
            "minecraft:pale_moss_patch", "minecraft:pale_garden_flowers",
            "minecraft:flower_pale_garden", "minecraft:patch_grass_forest",
            "minecraft:patch_pumpkin", "minecraft:patch_sugar_cane",
            "minecraft:patch_firefly_bush_near_water"
        };
        for (String w : want) {
            var holder = placed.getOrThrow(net.minecraft.resources.ResourceKey.create(
                    Registries.PLACED_FEATURE, net.minecraft.resources.Identifier.parse(w)));
            int idx = -1;
            for (int i = 0; i < step9.size(); i++) {
                if (step9.get(i) == holder.value()) { idx = i; break; }
            }
            System.out.println(w + " -> index " + idx);
        }
    }
}