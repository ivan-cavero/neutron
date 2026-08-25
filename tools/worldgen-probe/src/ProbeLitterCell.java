import java.util.ArrayList;
import java.util.HashSet;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Set;

import net.minecraft.SharedConstants;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Holder;
import net.minecraft.core.HolderLookup;
import net.minecraft.core.RegistryAccess;
import net.minecraft.core.SectionPos;
import net.minecraft.core.registries.Registries;
import net.minecraft.data.registries.VanillaRegistries;
import net.minecraft.resources.Identifier;
import net.minecraft.resources.ResourceKey;
import net.minecraft.server.Bootstrap;
import net.minecraft.world.level.ChunkPos;
import net.minecraft.world.level.LevelHeightAccessor;
import net.minecraft.world.level.WorldGenLevel;
import net.minecraft.world.level.biome.Biome;
import net.minecraft.world.level.biome.BiomeManager;
import net.minecraft.world.level.biome.FeatureSorter;
import net.minecraft.world.level.biome.MultiNoiseBiomeSource;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.chunk.ChunkAccess;
import net.minecraft.world.level.chunk.ChunkGenerator;
import net.minecraft.world.level.chunk.ProtoChunk;
import net.minecraft.world.level.chunk.LevelChunkSection;
import net.minecraft.world.level.chunk.PalettedContainerFactory;
import net.minecraft.world.level.chunk.LightChunkGetter;
import net.minecraft.world.level.chunk.UpgradeData;
import net.minecraft.world.ticks.ProtoChunkTicks;
import net.minecraft.world.level.dimension.DimensionType;
import net.minecraft.world.level.levelgen.NoiseBasedChunkGenerator;
import net.minecraft.world.level.levelgen.NoiseGeneratorSettings;
import net.minecraft.world.level.levelgen.RandomState;
import net.minecraft.world.level.levelgen.RandomSupport;
import net.minecraft.world.level.levelgen.WorldgenRandom;
import net.minecraft.world.level.levelgen.XoroshiroRandomSource;
import net.minecraft.world.level.levelgen.placement.PlacedFeature;
import net.minecraft.world.level.lighting.LevelLightEngine;

/**
 * WHO WROTE THE CARPET? Cell-exact attribution probe.
 *
 * Question: vanilla ref world seed 424242 has minecraft:leaf_litter at
 * (126,79,35) on grass_block@78 BEFORE chunk (7,2)'s dark_forest_vegetation
 * gif=17 ran (its attempt n0 at (126,35) dropped via SurfaceWaterDepthFilter,
 * world_surface 80 - ocean_floor 79 = 1). Which earlier origin + placed
 * feature wrote that leaf_litter?
 *
 * Method: replay applyBiomeDecoration over the NDEC1 dump for ALL 9 origins
 * of the 3x3 window around CC=(7,2) in ROW-MAJOR corner-first order
 * ((6,1),(7,1),(8,1),(6,2),(7,2),(8,2),(6,3),(7,3),(8,3)) exactly like
 * ProbeTreeAttempts / the parent brief. Every block write landing on
 * x==TX,z==TZ or inside the watch bbox is attributed to
 * (origin, step, globalIndex, placedFeatureId) by slicing ProbeDecorate.LOG
 * between features. Column (TX,TZ) is dumped after every origin.
 * Center pass additionally traces dark_forest_vegetation gif=17 through
 * ProbeTreeAttempts.traceFeature (exact modifier-chain staging) to confirm
 * the n0 drop reproduces here.
 */
public class ProbeLitterCell {
    static final int MINY = -64;
    static final int HEIGHT = 384;
    static final int TOP = MINY + HEIGHT;

    static final int TX = 126, TZ = 35;          // target cell column
    static final int BX0 = 112, BX1 = 132;       // watch bbox
    static final int BZ0 = 31, BZ1 = 39;
    static final int BY0 = 68, BY1 = 96;

    public static void main(String[] args) throws Exception {
        long SEED = Long.parseLong(args[0]);
        int CCX = Integer.parseInt(args[1]);
        int CCZ = Integer.parseInt(args[2]);
        String dumpPath = args[3];

        SharedConstants.tryDetectVersion();
        Bootstrap.bootStrap();
        ProbeDecorate.bindBlockTags();

        var lookup = VanillaRegistries.createLookup();
        ProbeTreeAttempts.LOOKUP = lookup;
        ProbeDecorate.REG_ACCESS = (RegistryAccess) ProbeDecorate.regAccessStub(lookup);

        var noises = lookup.lookupOrThrow(Registries.NOISE);
        var settingsHolder = lookup.lookupOrThrow(Registries.NOISE_SETTINGS)
                .getOrThrow(NoiseGeneratorSettings.OVERWORLD);
        RandomState rs = RandomState.create(settingsHolder.value(), noises, SEED);
        var plReg = lookup.lookupOrThrow(Registries.MULTI_NOISE_BIOME_SOURCE_PARAMETER_LIST);
        var plKey = ResourceKey.create(Registries.MULTI_NOISE_BIOME_SOURCE_PARAMETER_LIST,
                Identifier.parse("minecraft:overworld"));
        var biomeSource = MultiNoiseBiomeSource.createFromPreset(plReg.getOrThrow(plKey));
        ChunkGenerator generator = new NoiseBasedChunkGenerator(biomeSource, settingsHolder);
        ProbeDecorate.DIM_TYPE = lookup.lookupOrThrow(Registries.DIMENSION_TYPE)
                .getOrThrow(ResourceKey.create(Registries.DIMENSION_TYPE,
                        Identifier.parse("minecraft:overworld"))).value();
        BiomeManager biomeMgr = new BiomeManager(new BiomeManager.NoiseBiomeSource() {
            @Override
            public Holder<Biome> getNoiseBiome(int qx, int qy, int qz) {
                return biomeSource.getNoiseBiome(qx, qy, qz, rs.sampler());
            }
        }, BiomeManager.obfuscateSeed(SEED));
        ProbeDecorate.BIOME_MGR = biomeMgr;

        LinkedHashSet<Holder<Biome>> possible = new LinkedHashSet<>();
        String[] FIRST_SEEN = "mushroom_fields,deep_frozen_ocean,frozen_ocean,deep_cold_ocean,cold_ocean,deep_ocean,ocean,deep_lukewarm_ocean,lukewarm_ocean,warm_ocean,stony_shore,swamp,mangrove_swamp,snowy_slopes,snowy_plains,snowy_beach,windswept_gravelly_hills,grove,windswept_hills,snowy_taiga,windswept_forest,taiga,plains,meadow,beach,forest,old_growth_spruce_taiga,flower_forest,birch_forest,dark_forest,pale_garden,savanna_plateau,savanna,jungle,badlands,desert,wooded_badlands,jagged_peaks,stony_peaks,frozen_river,river,ice_spikes,old_growth_pine_taiga,sunflower_plains,old_growth_birch_forest,sparse_jungle,bamboo_jungle,eroded_badlands,windswept_savanna,cherry_grove,frozen_peaks,dripstone_caves,lush_caves,sulfur_caves,deep_dark".split(",");
        var biomeRegP = lookup.lookupOrThrow(Registries.BIOME);
        for (String bn : FIRST_SEEN) {
            possible.add(biomeRegP.getOrThrow(ResourceKey.create(Registries.BIOME,
                    Identifier.parse("minecraft:" + bn))));
        }
        for (var e : plReg.getOrThrow(plKey).value().parameters().values()) {
            possible.add(e.getSecond());
        }

        try {
            ProbeDecorate.SERVER_LEVEL_STUB = ProbePaleFlow.makeServerLevel(generator);
        } catch (Exception ee) {
            throw new RuntimeException("server level stub", ee);
        }

        ProbeDecorate.loadDump(dumpPath);

        var factory = ProbeDecorate.palettedContainerFactoryManual(lookup);
        LevelHeightAccessor lha = new LevelHeightAccessor() {
            @Override public int getHeight() { return HEIGHT; }
            @Override public int getMinY() { return MINY; }
            @Override public int getMaxY() { return TOP; }
            @Override public boolean isOutsideBuildHeight(int y) { return y < MINY || y >= TOP; }
            @Override public int getMinSectionY() { return MINY >> 4; }
            @Override public int getMaxSectionY() { return (TOP >> 4) - 1; }
            @Override public int getSectionsCount() { return HEIGHT / 16; }
            @Override public int getSectionIndex(int y) { return (y >> 4) - (MINY >> 4); }
            @Override public int getSectionIndexFromSectionY(int sy) { return sy - (MINY >> 4); }
        };
        final int R2 = 2, N2 = 5;
        ProtoChunk[][] chunks = new ProtoChunk[N2][N2];
        var biomeReg = lookup.lookupOrThrow(Registries.BIOME);
        for (int cz = 0; cz < N2; cz++) {
            for (int cx = 0; cx < N2; cx++) {
                var secs = new LevelChunkSection[HEIGHT / 16];
                byte[] grid = ProbeDecorate.QUART[cz][cx];
                int wx0 = ProbeDecorate.OX0 + cx * 16, wz0 = ProbeDecorate.OZ0 + cz * 16;
                for (int s = 0; s < HEIGHT / 16; s++) {
                    var statesC = factory.createForBlockStates();
                    var biomesC = factory.createForBiomes();
                    int baseY = MINY + s * 16;
                    for (int ly = 0; ly < 16; ly++) {
                        for (int lz = 0; lz < 16; lz++) {
                            for (int lx = 0; lx < 16; lx++) {
                                BlockState st = ProbeDecorate.store
                                        [wz0 - ProbeDecorate.OZ0 + lz][baseY + ly - MINY]
                                        [wx0 - ProbeDecorate.OX0 + lx];
                                if (!st.isAir()) {
                                    statesC.set(lx, ly, lz, st);
                                }
                            }
                        }
                    }
                    for (int sy = 0; sy < 4; sy++) {
                        for (int bz4 = 0; bz4 < 4; bz4++) {
                            for (int bx4 = 0; bx4 < 4; bx4++) {
                                int idx2 = s * 64 + sy * 16 + bz4 * 4 + bx4;
                                String bn = ProbeDecorate.BIOME_NAMES.get(grid[idx2] & 0xFF);
                                Holder<Biome> h = biomeReg.getOrThrow(ResourceKey.create(
                                        Registries.BIOME, Identifier.parse("minecraft:" + bn)));
                                biomesC.set(bx4, sy, bz4, h);
                            }
                        }
                    }
                    secs[s] = new LevelChunkSection(statesC, biomesC);
                }
                chunks[cz][cx] = new ProtoChunk(
                        new ChunkPos(CCX - R2 + cx, CCZ - R2 + cz),
                        UpgradeData.EMPTY, secs,
                        new ProtoChunkTicks<>(), new ProtoChunkTicks<>(),
                        lha, factory, null);
            }
        }
        ProbeDecorate.CHUNKS = chunks;

        WorldGenLevel level = (WorldGenLevel) java.lang.reflect.Proxy.newProxyInstance(
                ProbeLitterCell.class.getClassLoader(),
                new Class<?>[]{WorldGenLevel.class},
                (p, m, a) -> ProbeTreeAttempts.handleLevel(m.getName(), m.getReturnType(), a));

        ProbeDecorate.LIGHT = new LevelLightEngine(new LightChunkGetter() {
            @Override
            public net.minecraft.world.level.chunk.LightChunk getChunkForLighting(int x, int z) {
                return chunks[z][x];
            }
            @Override
            public net.minecraft.world.level.BlockGetter getLevel() {
                return (net.minecraft.world.level.BlockGetter) level;
            }
        }, true, false);

        var allBiomesList = new ArrayList<>(possible);
        var featuresPerStep = FeatureSorter.buildFeaturesPerStep(allBiomesList,
                b -> b.value().getGenerationSettings().features(), true);
        int featureStepCount = featuresPerStep.size();
        System.out.println("featureStepCount=" + featureStepCount + " seed=" + SEED
                + " center=(" + CCX + ',' + CCZ + ")");

        System.out.println("PRE-DECORATION column (" + TX + ",y," + TZ + "):");
        dumpColumn("  pre");

        int oi = 0;
        boolean carpetSeen = false;
        for (int ocz = CCZ - 1; ocz <= CCZ + 1; ocz++) {
            for (int ocx = CCX - 1; ocx <= CCX + 1; ocx++) {
                oi++;
                ChunkAccess center = ProbeDecorate.chunkAt(ocx, ocz);
                var sectionPos = SectionPos.of(center.getPos(), MINY >> 4);
                BlockPos origin = new BlockPos(
                        sectionPos.chunk().getMinBlockX(), MINY,
                        sectionPos.chunk().getMinBlockZ());
                int ORX = ocx * 16, ORZ = ocz * 16;
                ProbeDecorate.TAG_ORX = ORX;
                ProbeDecorate.TAG_ORZ = ORZ;
                ProbeDecorate.LEVEL_RANDOM = rs.getOrCreateRandomFactory(
                        Identifier.parse("minecraft:worldgen_region_random"))
                        .at(new BlockPos(ORX, 0, ORZ));

                Set<Holder<Biome>> possibleBiomes = new HashSet<>();
                ChunkPos.rangeClosed(sectionPos.chunk(), 1).forEach(chunkPos -> {
                    int qx = ChunkPos.getX(chunkPos.pack());
                    int qz = ChunkPos.getZ(chunkPos.pack());
                    ChunkAccess c = ProbeDecorate.chunkAt(qx, qz);
                    for (var section : c.getSections()) {
                        section.getBiomes().getAll(possibleBiomes::add);
                    }
                });
                possibleBiomes.retainAll(biomeSource.possibleBiomes());

                ProbeTreeAttempts.LoggingWgr random = new ProbeTreeAttempts.LoggingWgr(
                        new XoroshiroRandomSource(RandomSupport.generateUniqueSeed()));
                long decorationSeed = random.setDecorationSeed(SEED,
                        origin.getX(), origin.getZ());

                boolean centerPass = (ocx == CCX && ocz == CCZ);
                System.out.println("origin#" + oi + " (" + ocx + ',' + ocz + ") min=("
                        + origin.getX() + ',' + origin.getZ()
                        + ") decorationSeed=" + decorationSeed
                        + (centerPass ? "  <-- CENTER" : ""));

                for (int stepIndex = 0; stepIndex < featureStepCount; stepIndex++) {
                    var stepData = featuresPerStep.get(stepIndex);
                    Set<Integer> possibleThisStep = new HashSet<>();
                    for (Holder<Biome> biome : possibleBiomes) {
                        var featsInBiome = biome.value().getGenerationSettings().features();
                        if (stepIndex < featsInBiome.size()) {
                            for (var hf : featsInBiome.get(stepIndex)) {
                                possibleThisStep.add(
                                        stepData.indexMapping().applyAsInt(hf.value()));
                            }
                        }
                    }
                    int[] indexArray = possibleThisStep.stream()
                            .mapToInt(Integer::intValue).sorted().toArray();

                    for (int fi = 0; fi < indexArray.length; fi++) {
                        int gif = indexArray[fi];
                        PlacedFeature pf = stepData.features().get(gif);
                        String fname = ProbeTreeAttempts.idOfPlaced(pf);
                        random.setFeatureSeed(decorationSeed, gif, stepIndex);

                        if (centerPass && stepIndex == 9 && gif == 17) {
                            // exact staged replay of dark_forest_vegetation
                            ProbeTreeAttempts.traceFeature(fname, gif, pf, level,
                                    generator, random, origin);
                            reportHits("  [trace-gif17]", oi, ocx, ocz, stepIndex, gif, fname);
                            ProbeDecorate.syncSectionsToStore(ocx, ocz, gif, stepIndex);
                        } else {
                            int log0 = ProbeDecorate.LOG.length();
                            try {
                                pf.placeWithBiomeCheck(level, generator, random, origin);
                            } catch (Throwable t) {
                                System.out.println("ERROR placing gif=" + gif
                                        + " step=" + stepIndex + " origin=(" + ocx + ',' + ocz
                                        + "): " + t);
                            }
                            ProbeDecorate.syncSectionsToStore(ocx, ocz, gif, stepIndex);
                            String slice = ProbeDecorate.LOG.substring(log0);
                            reportSlice(slice, oi, ocx, ocz, stepIndex, gif, fname);
                        }
                        if (!carpetSeen && isCarpet()) {
                            carpetSeen = true;
                            System.out.println(">>> CARPET APPEARED: leaf_litter@(126,79,35)"
                                    + " after origin#" + oi + " (" + ocx + ',' + ocz
                                    + ") step=" + stepIndex + " gif=" + gif + " " + fname);
                        }
                    }
                }
                dumpColumn("  post-origin#"+oi+" ("+ocx+','+ocz+')');
            }
        }

        System.out.println("FINAL column (" + TX + ",y," + TZ + "):");
        dumpColumn("  final");
        System.out.println("carpet_present_final=" + isCarpet());
        System.out.println("total_writes=" + ProbeDecorate.WRITES);
        // per-attempt verdicts of the traced center gif17 (from ProbeTreeAttempts)
        System.out.println("--- ProbeTreeAttempts.traceFeature output (center gif17):");
        System.out.print(ProbeTreeAttempts.OUT);
    }

    static boolean isCarpet() {
        BlockState st = ProbeDecorate.getState(TX, 79, TZ);
        return net.minecraft.core.registries.BuiltInRegistries.BLOCK
                .getKey(st.getBlock()).toString().equals("minecraft:leaf_litter");
    }

    /** Attribute every logged write on the target cell / bbox to a feature. */
    static void reportSlice(String slice, int oi, int ocx, int ocz, int step, int gif,
                            String fname) {
        if (slice.isEmpty()) return;
        for (String line : slice.split("\n")) {
            String[] p = line.split("\\|");
            if (p.length < 6) continue;
            try {
                int x = Integer.parseInt(p[0]);
                int y = Integer.parseInt(p[1]);
                int z = Integer.parseInt(p[2]);
                boolean cellHit = (x == TX && z == TZ);
                boolean boxHit = x >= BX0 && x <= BX1 && z >= BZ0 && z <= BZ1
                        && y >= BY0 && y <= BY1;
                if (cellHit || boxHit) {
                    System.out.println("  WRITE o#" + oi + " (" + ocx + ',' + ocz
                            + ") step=" + step + " gif=" + gif + " " + fname
                            + " -> " + x + '|' + y + '|' + z + ' ' + p[3]
                            + (cellHit ? "  <<<< TARGET CELL" : ""));
                }
            } catch (NumberFormatException ignored) {}
        }
    }

    static void reportHits(String prefix, int oi, int ocx, int ocz, int step, int gif,
                           String fname) {
        // for the traced path, writes are already in LOG; scan only what the
        // trace just did by checking the live store delta is hard — instead we
        // re-report the current target-cell state transitions via isCarpet()
        // polling in main. Here we just print the bbox snapshot of interest.
        StringBuilder sb = new StringBuilder(prefix);
        sb.append(" traced ").append(fname).append(" done");
        System.out.println(sb);
    }

    static void dumpColumn(String tag) {
        StringBuilder sb = new StringBuilder(tag).append(" col(126,35):");
        for (int y = BY0; y <= BY1; y++) {
            BlockState st = ProbeDecorate.getState(TX, y, TZ);
            if (!st.isAir()) {
                sb.append(' ').append(y).append('=')
                  .append(net.minecraft.core.registries.BuiltInRegistries.BLOCK
                          .getKey(st.getBlock()).getPath());
            }
        }
        System.out.println(sb);
    }
}
