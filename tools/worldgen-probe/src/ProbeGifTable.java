import java.util.ArrayList;
import java.util.LinkedHashSet;
import java.util.List;

import net.minecraft.SharedConstants;
import net.minecraft.core.Holder;
import net.minecraft.core.registries.Registries;
import net.minecraft.data.registries.VanillaRegistries;
import net.minecraft.resources.Identifier;
import net.minecraft.resources.ResourceKey;
import net.minecraft.server.Bootstrap;
import net.minecraft.world.level.biome.Biome;
import net.minecraft.world.level.biome.FeatureSorter;
import net.minecraft.world.level.biome.MultiNoiseBiomeSource;
import net.minecraft.world.level.levelgen.placement.PlacedFeature;

/**
 * Dumps the FULL ordered placed-feature tables per generation step exactly as
 * ChunkGenerator.applyBiomeDecoration sees them (FeatureSorter.buildFeatures
 * PerStep over biomeSource.possibleBiomes()). Biome iteration order = the
 * real-server first-seen order hardcoded in ProbeTreeAttempts / neutron's
 * OVERWORLD_BIOME_ORDER (identical to preset order on 26.2).
 */
public class ProbeGifTable {
    static net.minecraft.core.HolderLookup.Provider LOOKUP;

    public static void main(String[] args) throws Exception {
        SharedConstants.tryDetectVersion();
        Bootstrap.bootStrap();
        var lookup = VanillaRegistries.createLookup();
        LOOKUP = lookup;

        LinkedHashSet<Holder<Biome>> possible = new LinkedHashSet<>();
        String[] FIRST_SEEN = "mushroom_fields,deep_frozen_ocean,frozen_ocean,deep_cold_ocean,cold_ocean,deep_ocean,ocean,deep_lukewarm_ocean,lukewarm_ocean,warm_ocean,stony_shore,swamp,mangrove_swamp,snowy_slopes,snowy_plains,snowy_beach,windswept_gravelly_hills,grove,windswept_hills,snowy_taiga,windswept_forest,taiga,plains,meadow,beach,forest,old_growth_spruce_taiga,flower_forest,birch_forest,dark_forest,pale_garden,savanna_plateau,savanna,jungle,badlands,desert,wooded_badlands,jagged_peaks,stony_peaks,frozen_river,river,ice_spikes,old_growth_pine_taiga,sunflower_plains,old_growth_birch_forest,sparse_jungle,bamboo_jungle,eroded_badlands,windswept_savanna,cherry_grove,frozen_peaks,dripstone_caves,lush_caves,sulfur_caves,deep_dark".split(",");
        var biomeRegP = lookup.lookupOrThrow(Registries.BIOME);
        for (String bn : FIRST_SEEN) {
            possible.add(biomeRegP.getOrThrow(ResourceKey.create(Registries.BIOME,
                    Identifier.parse("minecraft:" + bn))));
        }
        var plReg = lookup.lookupOrThrow(Registries.MULTI_NOISE_BIOME_SOURCE_PARAMETER_LIST);
        var plKey = ResourceKey.create(Registries.MULTI_NOISE_BIOME_SOURCE_PARAMETER_LIST,
                Identifier.parse("minecraft:overworld"));
        for (var e : plReg.getOrThrow(plKey).value().parameters().values()) {
            possible.add(e.getSecond());
        }

        // Cross-check: does preset possibleBiomes() iterate in the same order?
        var biomeSource = MultiNoiseBiomeSource.createFromPreset(plReg.getOrThrow(plKey));
        int i = 0;
        boolean orderMatches = true;
        for (var b : biomeSource.possibleBiomes()) {
            if (i >= possible.size() || !new ArrayList<>(possible).get(i).value().equals(b.value())) {
                orderMatches = false;
                System.out.println("ORDER-MISMATCH at " + i + ": preset=" + nameOf(b));
            }
            i++;
        }
        System.out.println("possibleBiomes count=" + i + " firstSeenCount=" + possible.size()
                + " presetOrderMatchesFirstSeen=" + orderMatches);

        var allBiomesList = new ArrayList<>(possible);
        var featuresPerStep = FeatureSorter.buildFeaturesPerStep(allBiomesList,
                b -> b.value().getGenerationSettings().features(), true);
        System.out.println("featureStepCount=" + featuresPerStep.size());
        for (int step = 0; step < featuresPerStep.size(); step++) {
            var feats = featuresPerStep.get(step).features();
            System.out.println("-- step " + step + " (" + feats.size() + " features) --");
            for (int g = 0; g < feats.size(); g++) {
                System.out.println("    " + g + "  " + nameOfPlaced(feats.get(g)));
            }
        }
    }

    static String nameOf(Holder<Biome> h) {
        return h.unwrapKey().map(k -> k.identifier().toString()).orElse("?");
    }

    static String nameOfPlaced(PlacedFeature pf) {
        var reg = LOOKUP.lookupOrThrow(Registries.PLACED_FEATURE);
        for (var e : reg.listElements().toList()) {
            if (e.value() == pf) return e.key().identifier().toString();
        }
        return "?";
    }
}
