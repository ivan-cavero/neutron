import java.util.List;
import net.minecraft.SharedConstants;
import net.minecraft.core.Holder;
import net.minecraft.core.registries.Registries;
import net.minecraft.server.Bootstrap;
import net.minecraft.world.level.biome.Biome;
import net.minecraft.world.level.levelgen.placement.PlacedFeature;
import net.minecraft.world.level.biome.FeatureSorter;

/** Print the REAL FeatureSorter step-9 (vegetal) indices. */
public class ProbeSorter9 {
    public static void main(String[] args) throws Exception {
        SharedConstants.tryDetectVersion();
        Bootstrap.bootStrap();
        var lookup = net.minecraft.data.registries.VanillaRegistries.createLookup();
        var placed = lookup.lookupOrThrow(Registries.PLACED_FEATURE);
        var paramList = lookup.lookupOrThrow(Registries.MULTI_NOISE_BIOME_SOURCE_PARAMETER_LIST)
                .getOrThrow(net.minecraft.resources.ResourceKey.create(
                        Registries.MULTI_NOISE_BIOME_SOURCE_PARAMETER_LIST,
                        net.minecraft.resources.Identifier.parse("minecraft:overworld")));
        var possible = paramList.value().parameters().values().stream()
                .map(com.mojang.datafixers.util.Pair::getSecond).distinct().toList();
        List<Holder<Biome>> all = new java.util.ArrayList<>(possible);
        var sorter = FeatureSorter.buildFeaturesPerStep(all, b -> b.value().getGenerationSettings().features(), true);
        var step9 = sorter.get(9).features();
        for (int i = 0; i < step9.size(); i++) {
            String name = "?";
            for (var e : placed.listElements().toList()) {
                if (e.value() == step9.get(i)) { name = e.key().identifier().toString(); break; }
            }
            System.out.println(i + " " + name);
        }
    }
}
