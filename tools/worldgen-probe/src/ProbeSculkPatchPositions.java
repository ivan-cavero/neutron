import java.util.List;
import net.minecraft.SharedConstants;
import net.minecraft.core.Holder;
import net.minecraft.core.registries.Registries;
import net.minecraft.server.Bootstrap;
import net.minecraft.world.level.biome.Biome;
import net.minecraft.world.level.biome.FeatureSorter;
import net.minecraft.world.level.levelgen.WorldgenRandom;
import net.minecraft.world.level.levelgen.XoroshiroRandomSource;
import net.minecraft.world.level.levelgen.placement.PlacedFeature;

/** Ground truth: sculk_patch_deep_dark position chain for one chunk origin.
 *  args: seed originX originZ  (block coords of chunk min corner) */
public class ProbeSculkPatchPositions {
    public static void main(String[] args) throws Exception {
        SharedConstants.tryDetectVersion();
        Bootstrap.bootStrap();
        long seed = Long.parseLong(args[0]);
        int ox = Integer.parseInt(args[1]);
        int oz = Integer.parseInt(args[2]);

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
        var sorter = FeatureSorter.buildFeaturesPerStep(all, b -> b.value().getGenerationSettings().features(), true);

        var wantKey = net.minecraft.resources.ResourceKey.create(
                Registries.PLACED_FEATURE,
                net.minecraft.resources.Identifier.parse("minecraft:sculk_patch_deep_dark"));
        Holder<PlacedFeature> want = placed.getOrThrow(wantKey);
        int idx = -1;
        var step7 = sorter.get(7).features();
        for (int i = 0; i < step7.size(); i++) {
            if (step7.get(i) == want.value()) { idx = i; break; }
        }
        System.out.println("sculk_patch_deep_dark step-7 index=" + idx);

        WorldgenRandom rng = new WorldgenRandom(new XoroshiroRandomSource(seed));
        long dec = rng.setDecorationSeed(seed, ox, oz);
        rng.setFeatureSeed(dec, idx, 7);
        for (int i = 0; i < 256; i++) {
            int x = rng.nextInt(16);
            int z = rng.nextInt(16);
            int y = -64 + rng.nextInt(321);
            System.out.println("i=" + i + " (" + (ox + x) + "," + y + "," + (oz + z) + ")");
        }
    }
}
