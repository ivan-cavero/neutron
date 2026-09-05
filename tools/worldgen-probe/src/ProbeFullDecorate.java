import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.lang.reflect.Proxy;
import java.util.ArrayList;
import java.util.HashSet;
import java.util.List;
import java.util.Set;
import net.minecraft.SharedConstants;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Holder;
import net.minecraft.core.HolderGetter;
import net.minecraft.core.HolderLookup;
import net.minecraft.core.QuartPos;
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
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.chunk.ChunkAccess;
import net.minecraft.world.level.chunk.LevelChunkSection;
import net.minecraft.world.level.chunk.ProtoChunk;
import net.minecraft.world.ticks.ProtoChunkTicks;
import net.minecraft.world.level.chunk.UpgradeData;
import net.minecraft.world.level.levelgen.LegacyRandomSource;
import net.minecraft.world.level.levelgen.NoiseBasedChunkGenerator;
import net.minecraft.world.level.levelgen.NoiseChunk;
import net.minecraft.world.level.levelgen.NoiseGeneratorSettings;
import net.minecraft.world.level.levelgen.RandomState;
import net.minecraft.world.level.levelgen.RandomSupport;
import net.minecraft.world.level.levelgen.WorldgenRandom;
import net.minecraft.world.level.levelgen.blending.Blender;
import net.minecraft.world.level.levelgen.carver.CarvingContext;
import net.minecraft.world.level.levelgen.carver.ConfiguredWorldCarver;
import net.minecraft.world.level.levelgen.DensityFunctions;
import net.minecraft.world.level.levelgen.feature.ConfiguredFeature;
import net.minecraft.world.level.levelgen.placement.PlacedFeature;

import net.minecraft.world.level.levelgen.synth.NormalNoise;
import net.minecraft.world.level.chunk.ChunkGenerator;
import net.minecraft.world.ticks.ProtoChunkTicks;

/**
 * FULL vanilla decoration replay over the REAL pre-decoration scene.
 *
 * Scene: fillFromNoise + buildSurface (real biomes from the NDEC1 dump) +
 * applyCarvers (real carver lists from the source biome). Then the real
 * applyBiomeDecoration loop for EVERY generation step (0..10), tracing
 * tree-producing placed features per attempt and running the rest as real
 * vanilla code.
 *
 * Output: the same ATTEMPT / ORIGIN / FEATURES lines as ProbeTreeFirstFlip,
 * prefixed with "STEP <n>" markers, for all steps.
 *
 * Usage: ProbeFullDecorate <seed> <ccx> <ccz> <dump.ndec> <ocx,ocz>...
 */
public class ProbeFullDecorate {
    static final int MINY = -64, TOP = 320, HEIGHT = TOP - MINY;
    static final int R = 2, N = 5;

    static long SEED;
    static int CCX, CCZ;
    static HolderLookup.Provider LOOKUP;
    static NoiseBasedChunkGenerator GEN;
    static RandomState RS;
    static List<FeatureSorter.StepFeatureData> FEATURES_PER_STEP;

    static Holder<Biome> PLAINS_HOLDER(BlockPos p) {
        // biome per position from the dump quart grid (real ref biomes)
        int lx = (p.getX() >> 4) - (CCX - R), lz = (p.getZ() >> 4) - (CCZ - R);
        if (lx < 0 || lz < 0 || lx >= N || lz >= N) return null;
        byte[] grid = ProbeDecorate.QUART[lz][lx];
        int sy = ((p.getY() - MINY) >> 2) / 4;
        sy = Math.max(0, Math.min(23, sy));
        int ly4 = ((p.getY() - MINY) >> 2) % 4;
        int idx2 = sy * 64 + ly4 * 16 + ((p.getZ() & 15) >> 2) * 4 + ((p.getX() & 15) >> 2);
        String bn = ProbeDecorate.BIOME_NAMES.get(grid[idx2] & 0xFF);
        return LOOKUP.lookupOrThrow(Registries.BIOME).getOrThrow(
                ResourceKey.create(Registries.BIOME, Identifier.parse("minecraft:" + bn)));
    }

    static java.util.HashMap<String, List<String>> TAG_RAW;
    static java.util.HashMap<String, List<String>> TAG_MEMO;
    static java.util.HashSet<String> TAG_RESOLVING;

    static List<String> resolveTag(String tag) {
        if (TAG_MEMO.containsKey(tag)) return TAG_MEMO.get(tag);
        if (!TAG_RESOLVING.add(tag)) return new ArrayList<>();
        List<String> out = new ArrayList<>();
        for (String v : TAG_RAW.getOrDefault(tag, new ArrayList<>())) {
            if (v.startsWith("#")) {
                String sub = v.substring(1);
                if (sub.startsWith("minecraft:")) sub = sub.substring(10);
                out.addAll(resolveTag(sub));
            } else {
                out.add(v);
            }
        }
        TAG_RESOLVING.remove(tag);
        TAG_MEMO.put(tag, out);
        return out;
    }
    public static void main(String[] args) throws Exception {
        SEED = Long.parseLong(args[0]);
        CCX = Integer.parseInt(args[1]);
        CCZ = Integer.parseInt(args[2]);
        String dumpPath = args[3];
        List<int[]> origins = new ArrayList<>();
        int rngTraceGif = -1;
        for (int i = 4; i < args.length; i++) {
            if (args[i].startsWith("gif=")) {
                rngTraceGif = Integer.parseInt(args[i].substring(4));
                continue;
            }
            String[] pp = args[i].split(",");
            origins.add(new int[]{Integer.parseInt(pp[0]), Integer.parseInt(pp[1])});
        }
        SharedConstants.tryDetectVersion();
        Bootstrap.bootStrap();
        LOOKUP = VanillaRegistries.createLookup();
        bindAllBlockTags();
        HolderGetter<NormalNoise.NoiseParameters> noises = LOOKUP.lookupOrThrow(Registries.NOISE);
        Holder<NoiseGeneratorSettings> settings =
                LOOKUP.lookupOrThrow(Registries.NOISE_SETTINGS).getOrThrow(NoiseGeneratorSettings.OVERWORLD);
        RS = RandomState.create(settings.value(), noises, SEED);
        var biomeReg = LOOKUP.lookupOrThrow(Registries.BIOME);

        // Load the dump ONLY for its biome grids (real ref biomes).
        ProbeDecorate.loadDump(dumpPath);
        var regAccess = (net.minecraft.core.RegistryAccess) ProbeDecorate.regAccessStub(LOOKUP);
        ProbeDecorate.REG_ACCESS = regAccess;
        ProbeTreeFirstFlip.LOOKUP = LOOKUP;


        var factory = ProbeDecorate.palettedContainerFactoryManual(LOOKUP);

        LevelHeightAccessor lha = new LevelHeightAccessor() {
            @Override public int getMinY() { return MINY; }
            @Override public int getHeight() { return HEIGHT; }
            @Override public int getMaxY() { return TOP; }
            @Override public boolean isOutsideBuildHeight(int y) { return y < MINY || y >= TOP; }
            @Override public int getMinSectionY() { return MINY >> 4; }
            @Override public int getMaxSectionY() { return (TOP >> 4) - 1; }
            @Override public int getSectionsCount() { return HEIGHT / 16; }
            @Override public int getSectionIndex(int y) { return (y >> 4) - (MINY >> 4); }
            @Override public int getSectionIndexFromSectionY(int sy) { return sy - (MINY >> 4); }
        };

        // Real overworld biome source + BIOME_MGR for the level proxy.
        var plReg = LOOKUP.lookupOrThrow(Registries.MULTI_NOISE_BIOME_SOURCE_PARAMETER_LIST);
        var plKey = ResourceKey.create(Registries.MULTI_NOISE_BIOME_SOURCE_PARAMETER_LIST,
                Identifier.parse("minecraft:overworld"));
        var realBiomeSource = net.minecraft.world.level.biome.MultiNoiseBiomeSource.createFromPreset(
                plReg.getOrThrow(plKey));
        ProbeDecorate.BIOME_MGR = new BiomeManager(new BiomeManager.NoiseBiomeSource() {
            @Override
            public Holder<Biome> getNoiseBiome(int qx, int qy, int qz) {
                return realBiomeSource.getNoiseBiome(qx, qy, qz, RS.sampler());
            }
        }, BiomeManager.obfuscateSeed(SEED));
        GEN = new NoiseBasedChunkGenerator(realBiomeSource, settings);

        BiomeManager biomeManager = new BiomeManager(new BiomeManager.NoiseBiomeSource() {
            @Override
            public Holder<Biome> getNoiseBiome(int qx, int qy, int qz) {
                return realBiomeSource.getNoiseBiome(qx, qy, qz, RS.sampler());
            }
        }, BiomeManager.obfuscateSeed(SEED));

        // ---- Scene: real fillFromNoise + buildSurface + applyCarvers ----
        ProtoChunk[][] chunks = new ProtoChunk[N][N];
        for (int cz = 0; cz < N; cz++) {
            for (int cx = 0; cx < N; cx++) {
                int ccx = CCX - R + cx, ccz = CCZ - R + cz;
                LevelChunkSection[] secs = new LevelChunkSection[HEIGHT / 16];
                byte[] grid = ProbeDecorate.QUART[cz][cx];
                for (int s = 0; s < secs.length; s++) {
                    var biomesC = factory.createForBiomes();
                    int baseY = MINY + s * 16;
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
                    secs[s] = new LevelChunkSection(factory.createForBlockStates(), biomesC);
                }
                chunks[cz][cx] = new ProtoChunk(new ChunkPos(ccx, ccz), UpgradeData.EMPTY, secs,
                        new ProtoChunkTicks<>(), new ProtoChunkTicks<>(), lha, factory, null);
            }
        }

        Field fpField = NoiseBasedChunkGenerator.class.getDeclaredField("globalFluidPicker");
        fpField.setAccessible(true);
        var fluid = (net.minecraft.world.level.levelgen.Aquifer.FluidPicker)
                ((java.util.function.Supplier<?>) fpField.get(GEN)).get();
        var beardCls = Class.forName("net.minecraft.world.level.levelgen.DensityFunctions$BeardifierMarker");
        var bf = beardCls.getField("INSTANCE");
        bf.setAccessible(true);
        var beard = (DensityFunctions.BeardifierOrMarker) bf.get(null);
        Field ncField = ChunkAccess.class.getDeclaredField("noiseChunk");
        ncField.setAccessible(true);
        var nsClamped = settings.value().noiseSettings().clampToHeightAccessor(lha);
        for (int cz = 0; cz < N; cz++) {
            for (int cx = 0; cx < N; cx++) {
                var nc = NoiseChunk.forChunk(chunks[cz][cx], RS, beard, settings.value(), fluid, Blender.empty());
                ncField.set(chunks[cz][cx], nc);
            }
        }
        for (int cz = 0; cz < N; cz++) {
            for (int cx = 0; cx < N; cx++) {
                ChunkAccess done = GEN.fillFromNoise(Blender.empty(), RS, null, chunks[cz][cx]).join();
                chunks[cz][cx] = (ProtoChunk) done;
            }
        }
        System.out.println("fillFromNoise done");

        var structureManagerNull = (net.minecraft.world.level.StructureManager) null;
        var wgc = new net.minecraft.world.level.levelgen.WorldGenerationContext(GEN, lha);
        Method bs = null;
        for (Method m : NoiseBasedChunkGenerator.class.getMethods()) {
            if (m.getName().equals("buildSurface") && m.getParameterCount() == 7) {
                bs = m; bs.setAccessible(true); break;
            }
        }
        var possibleBiomes = new HashSet<Holder<Biome>>();
        for (int cz = 1; cz < N - 1; cz++)
            for (int cx = 1; cx < N - 1; cx++)
                for (var section : chunks[cz][cx].getSections())
                    section.getBiomes().getAll(possibleBiomes::add);
        for (int cz = 1; cz < N - 1; cz++) {
            for (int cx = 1; cx < N - 1; cx++) {
                bs.invoke(GEN, chunks[cz][cx], wgc, RS, structureManagerNull,
                        biomeManager,
                        Blender.empty(), possibleBiomes);
            }
        }
        System.out.println("buildSurface done");

        // Carvers: real per-biome carver lists, real aquifer, mask per target.
        var carverReg = LOOKUP.lookupOrThrow(Registries.CONFIGURED_CARVER);
        var carverContextFix = settings.value().surfaceRule();
        for (int tzl = 1; tzl < N - 1; tzl++) {
            for (int txl = 1; txl < N - 1; txl++) {
                int tcx = CCX - R + txl, tcz = CCZ - R + tzl;
                ChunkAccess target = chunks[tzl][txl];
                NoiseChunk noiseChunk = target.getOrCreateNoiseChunk(c -> {
                    throw new IllegalStateException("cached");
                });
                CarvingContext context = new CarvingContext(GEN, regAccess, lha,
                        noiseChunk, RS, carverContextFix);
                var mask = new net.minecraft.world.level.chunk.CarvingMask(HEIGHT, MINY);
                var aquifer = noiseChunk.aquifer();
                var biomeGetter = (java.util.function.Function<BlockPos, Holder<Biome>>)
                        p -> PLAINS_HOLDER(p);
                for (int dx = -8; dx <= 8; dx++) {
                    for (int dz = -8; dz <= 8; dz++) {
                        int scx = tcx + dx, scz = tcz + dz;
                        if (Math.abs(scx - CCX) > R || Math.abs(scz - CCZ) > R) continue;
                        // source biome at center quart y=0 from the dump grid
                        int scxL = scx - (CCX - R), sczL = scz - (CCZ - R);
                        byte[] sgrid = ProbeDecorate.QUART[sczL][scxL];
                        int quartIdx = (0 >> 4) * 64 + 0 * 16 + 8; // section 0, sy=0? use center quart y=2 (y=8)
                        int syQ = (8 >> 2) & 3;
                        int idx2 = 0 * 64 + syQ * 16 + 2 * 4 + 2;
                        String sbn = ProbeDecorate.BIOME_NAMES.get(sgrid[idx2] & 0xFF);
                        Holder<Biome> sb = biomeReg.getOrThrow(ResourceKey.create(
                                Registries.BIOME, Identifier.parse("minecraft:" + sbn)));
                        var carvers = sb.value().getGenerationSettings().getCarvers();
                        WorldgenRandom random = new WorldgenRandom(new LegacyRandomSource(0L));
                        int index = 0;
                        for (var ch : carvers) {
                            ConfiguredWorldCarver<?> cw = ch.value();
                            random.setLargeFeatureSeed(SEED + index, scx, scz);
                            if (cw.isStartChunk(random)) {
                                cw.carve(context, target, biomeGetter, random, aquifer,
                                        new ChunkPos(scx, scz), mask);
                            }
                            index++;
                        }
                    }
                }
            }
        }
        System.out.println("applyCarvers done");

        // ---- Copy chunk sections into ProbeDecorate.store (the proxy scene) ----
        ProbeDecorate.CHUNKS = chunks;
        for (int cz = 0; cz < N; cz++) {
            for (int cx = 0; cx < N; cx++) {
                ChunkAccess ch = chunks[cz][cx];
                int wx0 = (CCX - R + cx) * 16, wz0 = (CCZ - R + cz) * 16;
                for (int s = 0; s < ch.getSectionsCount(); s++) {
                    var sec = ch.getSection(s);
                    int baseY = MINY + s * 16;
                    for (int ly = 0; ly < 16; ly++) {
                        for (int lz = 0; lz < 16; lz++) {
                            for (int lx = 0; lx < 16; lx++) {
                                BlockState st = sec.getBlockState(lx, ly, lz);
                                if (!st.isAir()) {
                                    ProbeDecorate.setState(wx0 + lx, baseY + ly, wz0 + lz, st);
                                }
                            }
                        }
                    }
                }
            }
        }
        System.out.println("scene synced to store");

        // ---- Decoration: all steps, real seeds, tree tracing ----
        WorldGenLevel level = (WorldGenLevel) Proxy.newProxyInstance(
                ProbeFullDecorate.class.getClassLoader(),
                new Class<?>[]{WorldGenLevel.class},
                (p, m, a) -> ProbeTreeFirstFlip.handleLevel(m.getName(), m.getReturnType(), a));

        // Vanilla builds featuresPerStep ONCE from biomeSource.possibleBiomes()
        // (the FULL set) — the FeatureSorter indices are global over that set.
        var allBiomesList = new ArrayList<>(GEN.getBiomeSource().possibleBiomes());
        FEATURES_PER_STEP = FeatureSorter.buildFeaturesPerStep(allBiomesList,
                b -> b.value().getGenerationSettings().features(), true);
        var generator = (ChunkGenerator) GEN;

        for (int[] oc : origins) {
            int ocx = oc[0], ocz = oc[1];
            ChunkAccess center = ProbeDecorate.chunkAt(ocx, ocz);
            var sectionPos = SectionPos.of(center.getPos(), MINY >> 4);
            BlockPos origin = new BlockPos(
                    sectionPos.chunk().getMinBlockX(), MINY, sectionPos.chunk().getMinBlockZ());
            ProbeDecorate.TAG_ORX = ocx * 16;
            ProbeDecorate.TAG_ORZ = ocz * 16;
            ProbeDecorate.LEVEL_RANDOM = RS.getOrCreateRandomFactory(
                    Identifier.parse("minecraft:worldgen_region_random"))
                    .at(new BlockPos(ocx * 16, 0, ocz * 16));

            Set<Holder<Biome>> originBiomes = new HashSet<>();
            ChunkPos.rangeClosed(sectionPos.chunk(), 1).forEach(chunkPos -> {
                ChunkAccess c = ProbeDecorate.chunkAt(ChunkPos.getX(chunkPos.pack()),
                        ChunkPos.getZ(chunkPos.pack()));
                for (var section : c.getSections()) {
                    section.getBiomes().getAll(originBiomes::add);
                }
            });

            System.out.println("ORIGIN " + ocx + " " + ocz);

            for (int step = 0; step < FEATURES_PER_STEP.size(); step++) {
                var stepData = FEATURES_PER_STEP.get(step);
                Set<Integer> possibleThisStep = new HashSet<>();
                for (Holder<Biome> biome : originBiomes) {
                    var featsInBiome = biome.value().getGenerationSettings().features();
                    if (step < featsInBiome.size()) {
                        for (var hf : featsInBiome.get(step)) {
                            possibleThisStep.add(stepData.indexMapping().applyAsInt(hf.value()));
                        }
                    }
                }
                int[] indexArray = possibleThisStep.stream().mapToInt(Integer::intValue).sorted().toArray();
                for (int gif : indexArray) {
                    PlacedFeature pf = stepData.features().get(gif);
                    ProbeTreeFirstFlip.LoggingWgr random = new ProbeTreeFirstFlip.LoggingWgr(
                            new net.minecraft.world.level.levelgen.XoroshiroRandomSource(
                                    RandomSupport.generateUniqueSeed()));
                    long decorationSeed = random.setDecorationSeed(SEED, origin.getX(), origin.getZ());
                    random.setFeatureSeed(decorationSeed, gif, step);
                    String fname = ProbeTreeFirstFlip.idOfPlaced(pf);
                    // RNG-STREAM capture for one gif (e.g. wildflowers gif=22):
                    // runs REAL vanilla placement (placeWithBiomeCheck) while
                    // logging every primitive draw — the faithful oracle the
                    // staged traceFeature probes cannot provide for
                    // multi-output modifiers (count fans).
                    if (step == 9 && gif == rngTraceGif) {
                        random.logging = true;
                        int log0 = ProbeDecorate.LOG.length();
                        try {
                            pf.placeWithBiomeCheck(level, generator, random, origin);
                        } catch (Throwable t) {
                            System.out.println("ERROR step=" + step + " gif=" + gif + " " + t);
                        }
                        random.logging = false;
                        System.out.print("STREAM gif=" + gif + " origin=" + ocx + "," + ocz
                                + " draws=" + ProbeTreeFirstFlip.tail(random.draws, 0) + "\n");
                        if (fname.contains("mangrove") && System.getenv("ROOTWALK") != null) {
                            // derive feature origin from the lowest mangrove_log write
                            int logY = Integer.MIN_VALUE; int lx = 0, lz = 0;
                            for (String ln : ProbeDecorate.LOG.substring(log0).split("\n")) {
                                String[] parts = ln.split("\\|");
                                if (parts.length >= 4 && parts[3].equals("minecraft:mangrove_log")) {
                                    int wy = Integer.parseInt(parts[1]);
                                    if (logY == Integer.MIN_VALUE || wy < logY) {
                                        logY = wy; lx = Integer.parseInt(parts[0]); lz = Integer.parseInt(parts[2]);
                                    }
                                }
                            }
                            if (logY != Integer.MIN_VALUE) {
                                java.util.List<Object> dr = random.draws;
                                int sel = -1;
                                for (int di = 0; di < dr.size(); di++) {
                                    if (dr.get(di) instanceof Float f && Math.abs(f - 0.6160519f) < 1e-4) { sel = di; break; }
                                }
                                if (sel >= 0 && sel + 4 <= dr.size()) {
                                    int offRes = (Integer) dr.get(sel + 3);
                                    int offY = 3 + offRes;
                                    ProbeMangroveRootTrace.traceWalk(level,
                                        new BlockPos(lx, logY - offY, lz),
                                        offY, dr.subList(sel + 4, dr.size()));
                                }
                            }
                        }
                        System.out.print(ProbeDecorate.LOG.substring(log0));
                        System.out.print("SCENECOL");
                        for (int yy = 60; yy <= 80; yy++) {
                            var stt = ProbeDecorate.getState(origin.getX() - 4, yy, origin.getZ() + 9);
                            System.out.print(" " + yy + ":" + net.minecraft.core.registries.BuiltInRegistries
                                .BLOCK.getKey(stt.getBlock()).getPath());
                        }
                        System.out.print("\n");
                        // Reflection dump of the placed feature's internal
                        // surface set: for VegetationPatchFeature the
                        // waterSurface (flooded interior) set is exactly what
                        // neutron's NEUTRON_PATCH_DUMP emits — the 1-point
                        // delta column falls out of this diff.
                        try {
                            Object cfgObj = ProbeTreeFirstFlip.fieldOf(
                                    pf.feature().value(), "config");
                            Object patch = ProbeTreeFirstFlip.fieldOf(cfgObj, "feature");
                            Object placed = patch.getClass().getMethod("value").invoke(patch);
                            Object feature = placed.getClass().getMethod("value").invoke(placed);
                            Object f = ProbeTreeFirstFlip.fieldOf(feature, "feature");
                            Object vf = f.getClass().getMethod("value").invoke(f);
                            java.lang.reflect.Field fsf = vf.getClass().getDeclaredField("surface");
                            fsf.setAccessible(true);
                            Object surfaceSet = fsf.get(vf);
                            System.out.print("SURFACE origin=" + ocx + "," + ocz + " set="
                                    + surfaceSet + "\n");
                        } catch (Throwable t) {
                            System.out.println("SURFACE origin=" + ocx + "," + ocz
                                    + " unavailable: " + t);
                        }
                        continue;
                    }
                    if (step == 9 && ProbeTreeFirstFlip.treeish(pf)) {
                        System.out.print("STEP " + step + " ORIGIN " + ocx + " " + ocz + "\n");
                        ProbeTreeFirstFlip.traceFeature(fname, gif, pf, level, generator, random, origin, ocx, ocz);
                        System.out.print(ProbeTreeFirstFlip.OUT);
                        ProbeTreeFirstFlip.OUT = new StringBuilder();
                    } else {
                        try {
                            pf.placeWithBiomeCheck(level, generator, random, origin);
                        } catch (Throwable t) {
                            System.out.println("ERROR step=" + step + " gif=" + gif + " " + t);
                        }
                    }
                }
            }
        }
        System.out.println("FULL DECORATION DONE");
    }
    /** Bind EVERY block tag from the server jar's data/ tags onto the
     *  registry holders — headless bootstrap leaves tags unbound and every
     *  state.is(TagKey) returns false, silently breaking feature gates. */
    static void bindAllBlockTags() throws Exception {
        var zip = new java.util.zip.ZipFile("tools/nbt-ref/vanilla-fresh-424242/versions/26.2/server-26.2.jar");
        var tagDir = "data/minecraft/tags/block/";
        TAG_RAW = new java.util.HashMap<>();
        TAG_MEMO = new java.util.HashMap<>();
        TAG_RESOLVING = new java.util.HashSet<>();
        var raw = TAG_RAW;
        var en = zip.entries();
        while (en.hasMoreElements()) {
            var e = en.nextElement();
            if (!e.isDirectory() && e.getName().startsWith(tagDir) && e.getName().endsWith(".json")) {
                String tag = e.getName().substring(tagDir.length(), e.getName().length() - 5);
                var is = zip.getInputStream(e);
                String txt = new String(is.readAllBytes(), java.nio.charset.StandardCharsets.UTF_8);
                is.close();
                var obj = com.google.gson.JsonParser.parseString(txt).getAsJsonObject();
                List<String> vals = new ArrayList<>();
                for (var v : obj.getAsJsonArray("values")) vals.add(v.getAsString());
                raw.put(tag, vals);
            }
        }
        var blockToTags = new java.util.HashMap<String, List<net.minecraft.tags.TagKey<net.minecraft.world.level.block.Block>>>();
        for (var entry : raw.entrySet()) {
            for (String b : resolveTag(entry.getKey())) {
                if (!b.startsWith("minecraft:")) b = "minecraft:" + b;
                blockToTags.computeIfAbsent(b, k -> new ArrayList<>())
                        .add(net.minecraft.tags.TagKey.create(Registries.BLOCK,
                                Identifier.parse("minecraft:" + entry.getKey())));
            }
        }
        var bind = Holder.Reference.class.getDeclaredMethod("bindTags", java.util.Collection.class);
        bind.setAccessible(true);
        int bound = 0;
        var blocks = net.minecraft.core.registries.BuiltInRegistries.BLOCK;
        for (var entry : blockToTags.entrySet()) {
            var id = Identifier.parse(entry.getKey());
            var holder = blocks.wrapAsHolder(blocks.getValue(id));
            if (holder instanceof Holder.Reference<?> ref) {
                bind.invoke(ref, entry.getValue());
                bound++;
            }
        }
        System.out.println("TAGS BOUND for " + bound + " blocks");
    }
}
