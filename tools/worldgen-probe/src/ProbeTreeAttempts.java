import java.lang.reflect.Field;
import java.util.ArrayList;
import java.util.HashSet;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Optional;
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
import net.minecraft.world.level.chunk.LightChunkGetter;
import net.minecraft.world.level.chunk.ProtoChunk;
import net.minecraft.world.level.chunk.PalettedContainerFactory;
import net.minecraft.world.level.chunk.LevelChunkSection;
import net.minecraft.world.level.chunk.UpgradeData;
import net.minecraft.world.ticks.ProtoChunkTicks;
import net.minecraft.world.level.dimension.DimensionType;
import net.minecraft.world.level.levelgen.Heightmap;
import net.minecraft.world.level.levelgen.NoiseBasedChunkGenerator;
import net.minecraft.world.level.levelgen.NoiseGeneratorSettings;
import net.minecraft.world.level.levelgen.RandomState;
import net.minecraft.world.level.levelgen.RandomSupport;
import net.minecraft.world.level.levelgen.WorldgenRandom;
import net.minecraft.world.level.levelgen.XoroshiroRandomSource;
import net.minecraft.world.level.levelgen.feature.ConfiguredFeature;
import net.minecraft.world.level.levelgen.feature.FallenTreeFeature;
import net.minecraft.world.level.levelgen.feature.TreeFeature;
import net.minecraft.world.level.levelgen.feature.WeightedPlacedFeature;
import net.minecraft.world.level.levelgen.feature.configurations.FallenTreeConfiguration;
import net.minecraft.world.level.levelgen.feature.configurations.RandomFeatureConfiguration;
import net.minecraft.world.level.levelgen.feature.configurations.TreeConfiguration;
import net.minecraft.world.level.levelgen.feature.trunkplacers.TrunkPlacer;
import net.minecraft.world.level.levelgen.placement.PlacedFeature;
import net.minecraft.world.level.levelgen.placement.PlacementContext;
import net.minecraft.world.level.levelgen.placement.PlacementModifier;
import net.minecraft.world.level.lighting.LevelLightEngine;
import net.minecraft.util.RandomSource;

/**
 * PER-ATTEMPT VANILLA ORACLE for two step-9 placed features
 * (minecraft:pale_garden_vegetation gif=13, minecraft:dark_forest_vegetation
 * gif=17) on one origin pass.
 *
 * Reuses ProbeDecorate's oracle internals (NDEC1 dump loader, block store,
 * proto chunks, level proxy, tag binding). Terrain input = NDEC1 dump
 * exported by crates/neutron-worldgen/examples/decorate_oracle.rs (noise +
 * surface + carvers + mineshafts); steps 0..9 are replayed here with the
 * REAL vanilla feature classes so the OCEAN_FLOOR / WORLD_SURFACE heightmaps
 * seen by each placement are live (earlier step-9 trees count).
 *
 * applyBiomeDecoration replication notes (26.2 decompile,
 * ChunkGenerator.java:321-327): decorationSeed =
 * WorldgenRandom.setDecorationSeed(levelSeed, origin.getX(), origin.getZ())
 * where origin = SectionPos.of(chunkPos, minSectionY).origin() = chunk MIN
 * CORNER in BLOCK coords. Per feature:
 * random.setFeatureSeed(decorationSeed, globalIndex, stepIndex); the SAME
 * random instance then drives the modifier chain and every configured-feature
 * place.
 *
 * For traced features the placement-modifier chain is staged manually against
 * that same random (consumption order identical to PlacedFeature
 * .placeWithContext's lazy flatMap chain: count -> per-position in_square ->
 * filters consume no rng), so each attempt's x/z/Y/biome and filter verdicts
 * can be reported. ConfiguredFeature.place then runs normally behind an
 * RNG-logging WorldgenRandom wrapper, from which we decode which
 * random_selector entry fired and the drawn tree height (the trunk placer's
 * two nextInt draws follow the selector floats immediately).
 */
public class ProbeTreeAttempts {
    static final int MINY = -64;
    static final int HEIGHT = 384;
    static final int TOP = MINY + HEIGHT;

    static long SEED;
    static int CCX, CCZ;
    static HolderLookup.Provider LOOKUP;
    static StringBuilder OUT = new StringBuilder();

    /** Logs every primitive draw while enabled; the underlying Xoroshiro
     *  source is shared, so state advances exactly like the real thing. */
    static class LoggingWgr extends WorldgenRandom {
        final List<Object> draws = new ArrayList<>();
        boolean logging;
        LoggingWgr(RandomSource src) { super(src); }
        @Override public int nextInt(int bound) {
            int v = super.nextInt(bound);
            if (logging) draws.add(v);
            return v;
        }
        @Override public float nextFloat() {
            float v = super.nextFloat();
            if (logging) draws.add(v);
            return v;
        }
        @Override public boolean nextBoolean() {
            boolean v = super.nextBoolean();
            if (logging) draws.add(v);
            return v;
        }
        @Override public long nextLong() {
            long v = super.nextLong();
            if (logging) draws.add(v);
            return v;
        }
        @Override public double nextDouble() {
            double v = super.nextDouble();
            if (logging) draws.add(v);
            return v;
        }
    }

    public static void main(String[] args) throws Exception {
        SEED = Long.parseLong(args[0]);
        CCX = Integer.parseInt(args[1]);
        CCZ = Integer.parseInt(args[2]);
        String dumpPath = args[3];

        SharedConstants.tryDetectVersion();
        Bootstrap.bootStrap();
        ProbeDecorate.bindBlockTags();

        var lookup = VanillaRegistries.createLookup();
        LOOKUP = lookup;
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

        // FeatureSorter order: possibleBiomes FIRST-SEEN order captured from a
        // real server (see ProbeDecorate comment / neutron OVERWORLD_BIOME_ORDER).
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
                LevelChunkSection[] secs = new LevelChunkSection[HEIGHT / 16];
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
                ProbeTreeAttempts.class.getClassLoader(),
                new Class<?>[]{WorldGenLevel.class},
                (p, m, a) -> handleLevel(m.getName(), m.getReturnType(), a));

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
        OUT.append("featureStepCount=").append(featureStepCount)
           .append(" seed=").append(SEED)
           .append(" center=(").append(CCX).append(',').append(CCZ).append(")\n");

        for (int ocz = CCZ - 1; ocz <= CCZ + 1; ocz++) {
            for (int ocx = CCX - 1; ocx <= CCX + 1; ocx++) {
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

                LoggingWgr random = new LoggingWgr(
                        new XoroshiroRandomSource(RandomSupport.generateUniqueSeed()));
                // vanilla passes the chunk min corner (BLOCK coords):
                // ChunkGenerator.applyBiomeDecoration line 327.
                long decorationSeed = random.setDecorationSeed(SEED,
                        origin.getX(), origin.getZ());

                boolean centerPass = (ocx == CCX && ocz == CCZ);
                OUT.append("origin ").append(ocx).append(',').append(ocz)
                   .append(" min=(").append(origin.getX()).append(',').append(origin.getZ())
                   .append(") decorationSeed=").append(decorationSeed)
                   .append(centerPass ? "  <-- CENTER" : "")
                   .append('\n');

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

                    if (centerPass && stepIndex == 9) {
                        OUT.append("  step9 global indices:");
                        for (int gif : indexArray) {
                            OUT.append(' ').append(gif).append('=')
                               .append(idOfPlaced(stepData.features().get(gif)));
                        }
                        OUT.append('\n');
                    }

                    for (int fi = 0; fi < indexArray.length; fi++) {
                        int gif = indexArray[fi];
                        PlacedFeature pf = stepData.features().get(gif);
                        random.setFeatureSeed(decorationSeed, gif, stepIndex);
                        boolean trace = centerPass && stepIndex == 9
                                && (gif == 13 || gif == 17 || gif == 22);
                        if (trace) {
                            traceFeature(idOfPlaced(pf), gif, pf, level, generator,
                                    random, origin);
                        } else {
                            try {
                                pf.placeWithBiomeCheck(level, generator, random, origin);
                            } catch (Throwable t) {
                                OUT.append("ERROR placing ").append(gif)
                                   .append(" step=").append(stepIndex)
                                   .append(" origin=(").append(ocx).append(',').append(ocz)
                                   .append("): ").append(t).append('\n');
                            }
                        }
                        ProbeDecorate.syncSectionsToStore(ocx, ocz, gif, stepIndex);
                    }
                }
            }
        }
        System.out.print(OUT);
        System.out.println("total_writes=" + ProbeDecorate.WRITES);
    }

    static String idOfPlaced(PlacedFeature pf) {
        var reg = LOOKUP.lookupOrThrow(Registries.PLACED_FEATURE);
        for (var e : reg.listElements().toList()) {
            if (e.value() == pf) {
                return e.key().identifier().getPath();
            }
        }
        return String.valueOf(pf);
    }

    static String biomeName(Holder<Biome> h) {
        return h.unwrapKey().map(k -> k.identifier().getPath()).orElse("?");
    }

    // ---------------- tracing ----------------

    /** Exact vanilla heightmap predicates (26.2 Heightmap.Types):
     *  WORLD_SURFACE(_WG)=!air · OCEAN_FLOOR(_WG)=blocksMotion ·
     *  MOTION_BLOCKING=blocksMotion||fluid · MOTION_BLOCKING_NO_LEAVES=
     *  (blocksMotion||fluid)&&!LeavesBlock. Scan top-down, return y+1. */
    static int heightOf(String type, int x, int z) {
        boolean worldSurface = type.startsWith("WORLD_SURFACE");
        boolean oceanFloor = type.startsWith("OCEAN_FLOOR");
        boolean noLeaves = type.endsWith("NO_LEAVES");
        for (int y = TOP - 1; y > MINY; y--) {
            BlockState s = ProbeDecorate.getState(x, y, z);
            boolean ok;
            if (worldSurface) ok = !s.isAir();
            else if (oceanFloor) ok = s.blocksMotion();
            else if (noLeaves)
                ok = (s.blocksMotion() || !s.getFluidState().isEmpty())
                        && !(s.getBlock() instanceof net.minecraft.world.level.block.LeavesBlock);
            else ok = s.blocksMotion() || !s.getFluidState().isEmpty();
            if (ok) return y + 1;
        }
        return MINY;
    }

    /** Proxy handler: exact height semantics for placement, then fall back
     *  to the shared ProbeDecorate oracle handler. */
    static Object handleLevel(String name, Class<?> ret, Object[] a) {
        switch (name) {
            case "getHeight": {
                if (a == null || a.length < 3) return HEIGHT;
                String type = a[0].toString();
                return heightOf(type, (Integer) a[1], (Integer) a[2]);
            }
            case "getHeightmapPos": {
                String type = a[0].toString();
                BlockPos bp = (BlockPos) a[1];
                return new BlockPos(bp.getX(),
                        heightOf(type, bp.getX(), bp.getZ()), bp.getZ());
            }
            default:
                return ProbeDecorate.handle(name, ret, a);
        }
    }

    static void traceFeature(String fname, int gif, PlacedFeature pf,
                             WorldGenLevel level, ChunkGenerator generator,
                             LoggingWgr random, BlockPos origin) throws Exception {
        OUT.append("== TRACE ").append(fname).append(" gif=").append(gif)
           .append(" originMin=(").append(origin.getX()).append(',').append(origin.getZ())
           .append(")\n");

        PlacementContext ctx = new PlacementContext(level, generator, Optional.of(pf));
        List<PlacementModifier> mods = pf.placement();

        // Stage 1: first modifier (= count) expands the input origin into N
        // attempt slots. ConstantInt consumes no RNG.
        List<BlockPos> slots = new ArrayList<>();
        for (BlockPos p : mods.get(0).getPositions(ctx, random, origin)
                .collect(java.util.stream.Collectors.toList())) {
            slots.add(p);
        }
        OUT.append("attempts=").append(slots.size())
           .append(" chain=").append(mods.size()).append('\n');

        // Stages 2..end are applied PER ATTEMPT, strictly interleaved with the
        // configured-feature place exactly like vanilla's lazy flatMap chain:
        // [in_square draws -> filters -> cf.place] for attempt k happen before
        // attempt k+1 draws, so earlier trees shift later heights AND consume
        // RNG that shifts later positions.
        ConfiguredFeature<?, ?> cf = pf.feature().value();
        Object cfgObj = fieldOf(cf, "config");
        boolean isSelector = cfgObj instanceof RandomFeatureConfiguration;
        List<WeightedPlacedFeature> entries = isSelector
                ? ((RandomFeatureConfiguration) cfgObj).features() : null;
        Holder<PlacedFeature> defaultFeat = isSelector
                ? ((RandomFeatureConfiguration) cfgObj).defaultFeature() : null;

        for (int i = 0; i < slots.size(); i++) {
            BlockPos p = slots.get(i);
            String verdict = null;
            Integer yOceanFloor = null;
            int wsAtCheck = -1, ofAtCheck = -1;
            String biomeAtPos = "?";

            for (int s = 1; s < mods.size() && verdict == null; s++) {
                PlacementModifier pm = mods.get(s);
                String pn = pm.getClass().getSimpleName();
                if (pn.equals("SurfaceWaterDepthFilter")) {
                    ofAtCheck = heightOf("OCEAN_FLOOR", p.getX(), p.getZ());
                    wsAtCheck = heightOf("WORLD_SURFACE", p.getX(), p.getZ());
                }
                List<BlockPos> outs = pm.getPositions(ctx, random, p)
                        .collect(java.util.stream.Collectors.toList());
                if (outs.isEmpty()) {
                    verdict = "DROP@" + pn;
                    break;
                }
                p = outs.get(0);
                if (pn.equals("HeightmapPlacement")) yOceanFloor = p.getY();
                if (pn.equals("BiomeFilter")) biomeAtPos = biomeName(level.getBiome(p));
            }

            int x = p.getX(), z = p.getZ();

            if (verdict != null) {
                int byY = yOceanFloor != null ? yOceanFloor
                        : Math.max(ofAtCheck, MINY + 1);
                OUT.append("ATTEMPT feat=").append(fname)
                   .append(" n=").append(i)
                   .append(" x=").append(x).append(" z=").append(z)
                   .append(" ocean_floor_Y=").append(yOceanFloor == null ? String.valueOf(ofAtCheck) : yOceanFloor)
                   .append(" world_surface_Y=").append(wsAtCheck < 0 ? "?" : wsAtCheck)
                   .append(" biome@pos=").append(biomeAtPos.equals("?")
                           ? biomeName(level.getBiome(new BlockPos(x, byY, z))) : biomeAtPos)
                   .append(" ").append(verdict);
                if (verdict.contains("SurfaceWaterDepthFilter")) {
                    StringBuilder col = new StringBuilder(" column=");
                    for (int y = wsAtCheck - 3; y <= wsAtCheck; y++) {
                        BlockState stq = ProbeDecorate.getState(x, y, z);
                        if (!stq.isAir()) {
                            col.append(y).append('=')
                               .append(net.minecraft.core.registries.BuiltInRegistries.BLOCK
                                       .getKey(stq.getBlock()))
                               .append(' ');
                        }
                    }
                    OUT.append(col);
                }
                OUT.append('\n');
                continue;
            }

            random.draws.clear();
            random.logging = true;
            long w0 = ProbeDecorate.WRITES;
            int log0 = ProbeDecorate.LOG.length();
            boolean ok;
            try {
                ok = cf.place(level, generator, random, p);
            } catch (Throwable t) {
                ok = false;
                OUT.append("PLACE ERROR attempt ").append(i).append(": ").append(t).append('\n');
            }
            random.logging = false;
            List<Object> draws = new ArrayList<>(random.draws);
            long nWrites = ProbeDecorate.WRITES - w0;
            String slice = ProbeDecorate.LOG.substring(log0);

            AttemptInfo info = decodeAttempt(isSelector, entries, defaultFeat, draws);
            OUT.append("ATTEMPT feat=").append(fname)
               .append(" n=").append(i)
               .append(" x=").append(x).append(" z=").append(z)
               .append(" ocean_floor_Y=").append(yOceanFloor == null ? "?" : yOceanFloor)
               .append(" world_surface_Y=").append(wsAtCheck < 0 ? "?" : wsAtCheck)
               .append(" biome@pos=").append(biomeAtPos)
               .append(info.text)
               .append(" place_ok=").append(ok)
               .append(" writes=").append(nWrites)
               .append(summarizeWrites(slice))
               .append('\n');
        }
    }

    static class AttemptInfo {
        String text = "";
    }

    /** Decode the random_selector choice (+ tree height draws) from the
     *  logged draw sequence. Draw order: one nextFloat per weighted entry
     *  until one satisfies float < chance; then the sub-feature runs: tree ->
     *  trunk placer nextInt(A+1), nextInt(B+1); fallen_tree -> log_length
     *  sample then nextInt(2); huge mushroom -> nothing. */
    static AttemptInfo decodeAttempt(boolean isSelector, List<WeightedPlacedFeature> entries,
                                     Holder<PlacedFeature> defaultFeat, List<Object> draws) {
        AttemptInfo info = new AttemptInfo();
        int p = 0;
        int chosen = -1;
        StringBuilder selLog = new StringBuilder();
        if (!isSelector) {
            info.text = " sel=(not-a-random-selector) draws_tail=" + tail(draws, 0);
            return info;
        }
        for (int e = 0; e < entries.size(); e++) {
            if (p >= draws.size() || !(draws.get(p) instanceof Float f)) {
                selLog.append(" DRAW-MISMATCH@").append(e);
                break;
            }
            p++;
            WeightedPlacedFeature wpf = entries.get(e);
            selLog.append(" f").append(e).append('=').append(f)
                  .append('<').append(wpf.chance());
            if (f < wpf.chance()) {
                chosen = e;
                break;
            }
        }
        Holder<PlacedFeature> subHolder = chosen >= 0
                ? entries.get(chosen).feature() : defaultFeat;
        String subName = subHolder.unwrapKey()
                .map(k -> k.identifier().getPath()).orElse("inline?");
        info.text = " sel=" + subName + selLog;

        ConfiguredFeature<?, ?> subCf;
        try {
            subCf = subHolder.value().feature().value();
        } catch (Throwable t) {
            return info;
        }
        Object subCfgObj = fieldOf(subCf, "config");
        if (subCf.feature() instanceof TreeFeature && subCfgObj instanceof TreeConfiguration tc) {
            TrunkPlacer tp = fieldOf(tc, "trunkPlacer");
            int base = intField(tp, "baseHeight");
            int a = intField(tp, "heightRandA");
            int b = intField(tp, "heightRandB");
            Integer d1 = p < draws.size() && draws.get(p) instanceof Integer i1 ? i1 : null;
            Integer d2 = p + 1 < draws.size() && draws.get(p + 1) instanceof Integer i2 ? i2 : null;
            if (d1 != null && d2 != null) {
                info.text += " tree_height=" + (base + d1 + d2)
                        + " (" + base + "+" + d1 + "+" + d2 + ")";
            } else {
                info.text += " tree_height=? draws_tail=" + tail(draws, p);
            }
        } else if (subCf.feature() instanceof FallenTreeFeature
                && subCfgObj instanceof FallenTreeConfiguration fc) {
            Object prov = fc.logLength;
            int mn = intField(prov, "minInclusive");
            Integer d1 = p < draws.size() && draws.get(p) instanceof Integer i1 ? i1 : null;
            if (d1 != null) {
                info.text += " log_length=" + (mn + d1)
                        + " draws_tail=" + tail(draws, p + 1);
            } else {
                info.text += " log_length=?";
            }
        } else if (isSelector) {
            info.text += " extra_draws=" + tail(draws, p);
        }
        return info;
    }

    static String tail(List<Object> draws, int from) {
        StringBuilder sb = new StringBuilder("[");
        for (int i = from; i < draws.size(); i++) {
            if (i > from) sb.append(',');
            sb.append(draws.get(i));
        }
        return sb.append(']').toString();
    }

    static int intField(Object o, String name) {
        try {
            Field f = findField(o.getClass(), name);
            f.setAccessible(true);
            return f.getInt(o);
        } catch (Exception e) {
            throw new RuntimeException("field " + name + " on " + o.getClass(), e);
        }
    }

    @SuppressWarnings("unchecked")
    static <T> T fieldOf(Object o, String name) {
        try {
            Field f = findField(o.getClass(), name);
            f.setAccessible(true);
            return (T) f.get(o);
        } catch (Exception e) {
            throw new RuntimeException("field " + name + " on " + o.getClass(), e);
        }
    }

    static Field findField(Class<?> c, String name) throws NoSuchFieldException {
        while (c != null) {
            try {
                return c.getDeclaredField(name);
            } catch (NoSuchFieldException nsf) {
                c = c.getSuperclass();
            }
        }
        throw new NoSuchFieldException(name);
    }

    /** Compact summary of the block writes made by one attempt. */
    static String summarizeWrites(String logSlice) {
        if (logSlice.isEmpty()) return "";
        java.util.Map<String, Integer> counts = new java.util.TreeMap<>();
        int minX = Integer.MAX_VALUE, minY = Integer.MAX_VALUE, minZ = Integer.MAX_VALUE;
        int maxX = Integer.MIN_VALUE, maxY = Integer.MIN_VALUE, maxZ = Integer.MIN_VALUE;
        int n = 0;
        for (String line : logSlice.split("\n")) {
            String[] parts = line.split("\\|");
            if (parts.length < 5) continue;
            try {
                int px = Integer.parseInt(parts[0]);
                int py = Integer.parseInt(parts[1]);
                int pz = Integer.parseInt(parts[2]);
                minX = Math.min(minX, px); maxX = Math.max(maxX, px);
                minY = Math.min(minY, py); maxY = Math.max(maxY, py);
                minZ = Math.min(minZ, pz); maxZ = Math.max(maxZ, pz);
            } catch (NumberFormatException ignored) {}
            String blk = parts[3].replace("minecraft:", "");
            counts.merge(blk, 1, Integer::sum);
            n++;
        }
        StringBuilder sb = new StringBuilder(" bbox=[");
        sb.append(minX).append(',').append(minY).append(',').append(minZ).append("..")
          .append(maxX).append(',').append(maxY).append(',').append(maxZ).append(']');
        sb.append(" blocks={");
        boolean first = true;
        for (var en : counts.entrySet()) {
            if (!first) sb.append(',');
            sb.append(en.getKey()).append(':').append(en.getValue());
            first = false;
        }
        sb.append('}');
        return sb.toString();
    }
}
