import java.util.ArrayList;
import java.util.List;
import java.util.HashSet;
import java.util.Set;
import java.util.TreeSet;
import net.minecraft.SharedConstants;
import net.minecraft.core.Holder;
import net.minecraft.core.registries.Registries;
import net.minecraft.data.registries.VanillaRegistries;
import net.minecraft.resources.Identifier;
import net.minecraft.resources.ResourceKey;
import net.minecraft.server.Bootstrap;
import net.minecraft.util.Mth;
import net.minecraft.world.level.biome.Biome;
import net.minecraft.world.level.biome.Climate;
import net.minecraft.world.level.biome.FeatureSorter;
import net.minecraft.world.level.biome.MultiNoiseBiomeSource;
import net.minecraft.world.level.biome.MultiNoiseBiomeSourceParameterLists;
import net.minecraft.world.level.levelgen.GenerationStep;
import net.minecraft.world.level.levelgen.NoiseGeneratorSettings;
import net.minecraft.world.level.levelgen.RandomState;
import net.minecraft.world.level.levelgen.WorldgenRandom;
import net.minecraft.world.level.levelgen.XoroshiroRandomSource;
import net.minecraft.world.level.levelgen.placement.PlacedFeature;
import net.minecraft.util.valueproviders.UniformInt;

/**
 * AgentF: TRUE global FeatureSorter index of minecraft:amethyst_geode (26.2),
 * per-origin stability demo, and a gif x origin sweep for seed 424242.
 *
 * 1. Global step lists from FeatureSorter.buildFeaturesPerStep over
 *    BiomeSource.possibleBiomes() (exactly what ChunkGenerator memoizes).
 * 2. applyBiomeDecoration emulation for sample origins: 3x3 section biomes
 *    -> possibleFeaturesThisStep = sorted global indices -> shows the SEED
 *    INDEX of a feature never varies per origin (indexMapping is identity
 *    into the GLOBAL list); only the fired SUBSET varies.
 * 3. For every nearby origin and candidate gif in step 2, replay
 *    rarity(1/24) -> in_square -> uniform height -> GeodeFeature head draws,
 *    printing anchors so the ref-world geodes can be attributed.
 */
public class ProbeGeodeSweep {
    static List<PlacedFeature> step2;
    static FeatureSorter.StepFeatureData stepData;

    public static void main(String[] args) throws Exception {
        long seed = Long.parseLong(args[0]);
        SharedConstants.tryDetectVersion();
        Bootstrap.bootStrap();
        var lookup = VanillaRegistries.createLookup();
        var preset = lookup.lookupOrThrow(Registries.MULTI_NOISE_BIOME_SOURCE_PARAMETER_LIST)
            .getOrThrow(MultiNoiseBiomeSourceParameterLists.OVERWORLD);
        var biomeSource = MultiNoiseBiomeSource.createFromPreset(preset);

        // ---- 1. global order over ALL possible biomes (ChunkGenerator ctor) ----
        List<FeatureSorter.StepFeatureData> steps = FeatureSorter.buildFeaturesPerStep(
            List.copyOf(biomeSource.possibleBiomes()),
            h -> h.value().getGenerationSettings().features(), true);
        stepData = steps.get(2);
        step2 = stepData.features();
        System.out.println("=== step 2 (LOCAL_MODIFICATIONS) global list ===");
        int geodeGif = -1;
        for (int i = 0; i < step2.size(); i++) {
            String id = idOf(lookup, step2.get(i));
            if (id.equals("minecraft:amethyst_geode")) geodeGif = i;
            System.out.printf("  %d  %s%n", i, id);
        }
        System.out.println("AMETHYST_GEODE_GLOBAL_INDEX(step=2)=" + geodeGif);

        // noise settings for climate sampling
        var noises = lookup.lookupOrThrow(Registries.NOISE);
        var settings = lookup.lookupOrThrow(Registries.NOISE_SETTINGS).getOrThrow(NoiseGeneratorSettings.OVERWORLD);
        RandomState rs = RandomState.create(settings.value(), noises, seed);
        Climate.Sampler sampler = rs.sampler();

        // ---- 2. per-origin subset demo ----
        int[][] demoOrigins = {{-11,-1},{-11,-3},{-10,1},{0,0},{5,7}};
        for (int[] c : demoOrigins) {
            Set<String> biomes = biomeUnion(biomeSource, sampler, c[0], c[1]);
            TreeSet<Integer> idx = new TreeSet<>();
            for (Holder<Biome> b : biomeSource.possibleBiomes()) {
                if (!biomes.contains(idOf(biomeName(b)))) continue;
                var feats = b.value().getGenerationSettings().features();
                if (2 < feats.size()) {
                    for (var hf : feats.get(2)) {
                        idx.add(stepData.indexMapping().applyAsInt(hf.value()));
                    }
                }
            }
            StringBuilder sb = new StringBuilder("origin chunk (" + c[0] + "," + c[1] + ") step2 fired gifs:");
            for (int g : idx) sb.append(' ').append(g).append('=').append(idOf(lookup, step2.get(g)));
            System.out.println(sb);
        }

        // ---- 3. gif x origin sweep ----
        System.out.println("=== sweep: seed " + seed + " step 2 ===");
        System.out.println("format: cx cz gif fire px py pz numPoints crackSize");
        for (int cx = -14; cx <= 12; cx++) {
            for (int cz = -8; cz <= 14; cz++) {
                for (int gif = 0; gif < step2.size(); gif++) {
                    replay(seed, cx, cz, gif);
                }
            }
        }
    }

    static String biomeName(Holder<Biome> b) {
        return b.unwrapKey().map(k -> k.identifier().toString()).orElse("?");
    }

    /** 3x3-chunk biome union at section resolution via noise biome sampling
     *  (vanilla collects section biomes of loaded neighbours; same set). */
    static Set<String> biomeUnion(MultiNoiseBiomeSource src, Climate.Sampler smp, int cx, int cz) {
        Set<String> out = new HashSet<>();
        for (int dx = -1; dx <= 1; dx++) {
            for (int dz = -1; dz <= 1; dz++) {
                int bx = (cx + dx) * 16, bz = (cz + dz) * 16;
                for (int sec = 0; sec < 24; sec++) {
                    int y0 = -64 + sec * 16;
                    for (int sy4 = 0; sy4 < 4; sy4++)
                        for (int zq = 0; zq < 4; zq++)
                            for (int xq = 0; xq < 4; xq++) {
                                Holder<Biome> b = src.getNoiseBiome(
                                    (bx >> 2) + xq, (y0 >> 2) + sy4, (bz >> 2) + zq, smp);
                                out.add(biomeName(b));
                            }
                }
            }
        }
        return out;
    }

    static void replay(long seed, int cx, int cz, int gif) {
        WorldgenRandom rng = new WorldgenRandom(new XoroshiroRandomSource(seed));
        long dec = rng.setDecorationSeed(seed, cx * 16, cz * 16);
        rng.setFeatureSeed(dec, gif, 2);
        float roll = rng.nextFloat();
        if (!(roll < 1.0f / 24.0f)) return;
        int px = cx * 16 + rng.nextInt(16);
        int pz = cz * 16 + rng.nextInt(16);
        int py = Mth.randomBetweenInclusive(rng, -58, 30);
        UniformInt distPoints = UniformInt.of(3, 4);
        UniformInt wallDist = UniformInt.of(4, 6);
        int numPoints = distPoints.sample(rng);
        double adj = (double) numPoints / 6.0;
        double crackSize = 1.0 / Math.sqrt(2.0 + rng.nextDouble() / 2.0 + (numPoints > 3 ? adj : 0.0));
        boolean crack = rng.nextFloat() < 0.95f;
        System.out.printf("%d %d %d %d %d %d %d %d %.4f %b%n",
            cx, cz, gif, 1, px, py, pz, numPoints, crackSize, crack);
    }

    static String idOf(String name) { return name; }

    static String idOf(net.minecraft.core.HolderLookup.Provider lookup, PlacedFeature f) {
        var reg = lookup.lookupOrThrow(Registries.PLACED_FEATURE);
        for (var e : reg.listElements().toList()) {
            if (e.value() == f) return e.key().identifier().toString();
        }
        return "?";
    }
}
