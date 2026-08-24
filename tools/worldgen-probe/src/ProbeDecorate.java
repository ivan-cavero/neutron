import java.io.DataInputStream;
import java.io.FileInputStream;
import java.lang.reflect.Proxy;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.HashSet;
import java.util.List;
import java.util.Set;

import net.minecraft.SharedConstants;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Registry;
import net.minecraft.world.level.chunk.Strategy;
import java.util.Iterator;
import net.minecraft.core.SectionPos;
import net.minecraft.core.Holder;
import net.minecraft.core.HolderLookup;
import net.minecraft.core.RegistryAccess;
import net.minecraft.core.registries.Registries;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.data.registries.VanillaRegistries;
import net.minecraft.resources.ResourceKey;
import net.minecraft.resources.Identifier;
import net.minecraft.server.Bootstrap;
import net.minecraft.world.level.ChunkPos;
import net.minecraft.world.level.Level;
import net.minecraft.world.level.LevelHeightAccessor;
import net.minecraft.world.level.LevelAccessor;
import net.minecraft.world.level.BlockGetter;
import net.minecraft.world.level.WorldGenLevel;
import net.minecraft.world.level.biome.Biome;
import net.minecraft.tags.TagLoader;
import net.minecraft.world.level.biome.BiomeManager;
import net.minecraft.world.level.biome.FeatureSorter;
import net.minecraft.world.level.biome.MultiNoiseBiomeSource;
import net.minecraft.world.level.biome.MultiNoiseBiomeSourceParameterList;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.Blocks;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.levelgen.feature.ConfiguredFeature;
import net.minecraft.world.level.chunk.ChunkAccess;
import net.minecraft.world.level.chunk.LevelChunkSection;
import net.minecraft.world.level.chunk.LightChunk;
import net.minecraft.world.level.chunk.PalettedContainerFactory;
import net.minecraft.world.level.chunk.ProtoChunk;
import net.minecraft.world.ticks.ProtoChunkTicks;
import net.minecraft.world.level.chunk.LightChunkGetter;
import net.minecraft.world.level.chunk.UpgradeData;
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
 * VANILLA DECORATION ORACLE.
 *
 * Feeds a pre-decoration terrain snapshot (noise + surface + carvers +
 * mineshafts; exported by examples/export_predecorate.rs, NDEC1 format) into
 * vanilla's REAL placed-feature classes by replicating
 * ChunkGenerator.applyBiomeDecoration's exact loop (biome union from stored
 * sections, FeatureSorter global indices, per-(index,step) seeds,
 * placeWithBiomeCheck). Every block write is recorded tagged by origin chunk.
 *
 * On identical input terrain any difference between these writes and
 * neutron's decoration output is a pure feature-logic bug.
 */
public class ProbeDecorate {
    static final int RADIUS = 2;
    static final int N = RADIUS * 2 + 1;
    static final int SIDE = N * 16;
    static final int MINY = -64;
    static final int HEIGHT = 384;
    static final int TOP = MINY + HEIGHT;

    static BlockState[][][] store; // [z][y][x]
    static long SEED;
    static int CCX, CCZ, OX0, OZ0;
    static List<String> BIOME_NAMES = new ArrayList<>();
    static byte[][][] QUART;
    static ProtoChunk[][] CHUNKS;
    static BiomeManager BIOME_MGR;
    static DimensionType DIM_TYPE;
    static RegistryAccess REG_ACCESS;
    public static int TAG_ORX, TAG_ORZ;
    static LevelLightEngine LIGHT;
    static Object LEVEL_PROXY;
    static Object levelProxy;
        static long WRITES = 0;
    static StringBuilder LOG = new StringBuilder();
    static int WATCH_COUNT = 0;

    public static void main(String[] args) throws Exception {
        SEED = Long.parseLong(args[0]);
        CCX = Integer.parseInt(args[1]);
        CCZ = Integer.parseInt(args[2]);
        String dumpPath = args[3];
        String outPath = args.length > 4 ? args[4] : null;

        SharedConstants.tryDetectVersion();
        Bootstrap.bootStrap();
    bindBlockTags();
        var lookup = VanillaRegistries.createLookup();
        REG_ACCESS = (RegistryAccess) regAccessStub(lookup);

        var noises = lookup.lookupOrThrow(Registries.NOISE);
        var settingsHolder = lookup.lookupOrThrow(Registries.NOISE_SETTINGS)
                .getOrThrow(NoiseGeneratorSettings.OVERWORLD);
        RandomState rs = RandomState.create(settingsHolder.value(), noises, SEED);
        var plReg = lookup.lookupOrThrow(Registries.MULTI_NOISE_BIOME_SOURCE_PARAMETER_LIST);
        var plKey = ResourceKey.create(Registries.MULTI_NOISE_BIOME_SOURCE_PARAMETER_LIST,
                Identifier.parse("minecraft:overworld"));
        var biomeSource = MultiNoiseBiomeSource.createFromPreset(plReg.getOrThrow(plKey));
        var generator = new NoiseBasedChunkGenerator(biomeSource, settingsHolder);
        DIM_TYPE = lookup.lookupOrThrow(Registries.DIMENSION_TYPE)
                .getOrThrow(ResourceKey.create(Registries.DIMENSION_TYPE,
                        Identifier.parse("minecraft:overworld"))).value();
        BIOME_MGR = new BiomeManager(new BiomeManager.NoiseBiomeSource() {
            @Override
            public Holder<Biome> getNoiseBiome(int qx, int qy, int qz) {
                return biomeSource.getNoiseBiome(qx, qy, qz, rs.sampler());
            }
        }, BiomeManager.obfuscateSeed(SEED));

        Set<Holder<Biome>> possible = new HashSet<>(plReg.getOrThrow(plKey).value()
                .parameters().values().stream()
                .map(com.mojang.datafixers.util.Pair::getSecond).distinct().toList());

        loadDump(dumpPath);

        var factory = palettedContainerFactoryManual(lookup);
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
        CHUNKS = new ProtoChunk[N][N];
        var biomeReg = lookup.lookupOrThrow(Registries.BIOME);
        for (int cz = 0; cz < N; cz++) {
            for (int cx = 0; cx < N; cx++) {
                LevelChunkSection[] secs = new LevelChunkSection[HEIGHT / 16];
                byte[] grid = QUART[cz][cx];
                int wx0 = OX0 + cx * 16, wz0 = OZ0 + cz * 16;
                for (int s = 0; s < HEIGHT / 16; s++) {
                    var statesC = factory.createForBlockStates();
                    var biomesC = factory.createForBiomes();
                    int baseY = MINY + s * 16;
                    for (int ly = 0; ly < 16; ly++) {
                        for (int lz = 0; lz < 16; lz++) {
                            for (int lx = 0; lx < 16; lx++) {
                                BlockState st =
                                        store[wz0 - OZ0 + lz][baseY + ly - MINY][wx0 - OX0 + lx];
                                if (!st.isAir()) {
                                    statesC.set(lx, ly, lz, st);
                                }
                            }
                        }
                    }
                    int baseYq = baseY >> 2;
                    for (int sy = 0; sy < 4; sy++) {
                        for (int bz4 = 0; bz4 < 4; bz4++) {
                            for (int bx4 = 0; bx4 < 4; bx4++) {
                                int idx2 = s * 64 + sy * 16 + bz4 * 4 + bx4;
                                String bn = BIOME_NAMES.get(grid[idx2] & 0xFF);
                                Holder<Biome> h = biomeReg.getOrThrow(ResourceKey.create(
                                        Registries.BIOME,
                                        Identifier.parse("minecraft:" + bn)));
                                biomesC.set(bx4, sy, bz4, h);
                            }
                        }
                    }
                    secs[s] = new LevelChunkSection(statesC, biomesC);
                }
                CHUNKS[cz][cx] = new ProtoChunk(
                        new ChunkPos(CCX - RADIUS + cx, CCZ - RADIUS + cz),
                        UpgradeData.EMPTY, secs,
                        new ProtoChunkTicks<>(), new ProtoChunkTicks<>(),
                        lha, factory, null);
            }
        }

        LIGHT = new LevelLightEngine(new LightChunkGetter() {
            @Override
            public LightChunk getChunkForLighting(int x, int z) {
                return CHUNKS[z][x];
            }
            @Override
            public BlockGetter getLevel() {
                return (BlockGetter) levelProxy;
            }
        }, true, false);

        levelProxy = Proxy.newProxyInstance(
                ProbeDecorate.class.getClassLoader(),
                new Class<?>[]{WorldGenLevel.class},
                (p, m, a) -> handle(m.getName(), m.getReturnType(), a));

        // ---- replicate ChunkGenerator.applyBiomeDecoration per origin ----
        var allBiomesList = new java.util.ArrayList<>(possible);
        var featuresPerStep = FeatureSorter.buildFeaturesPerStep(allBiomesList,
                b -> b.value().getGenerationSettings().features(), true);
        int featureStepCount = featuresPerStep.size();

        for (int ocz = CCZ - 1; ocz <= CCZ + 1; ocz++) {
            for (int ocx = CCX - 1; ocx <= CCX + 1; ocx++) {
                ChunkAccess center = chunkAt(ocx, ocz);
                var sectionPos = SectionPos.of(center.getPos(), MINY >> 4);
                BlockPos origin = new BlockPos(
                        sectionPos.chunk().getMinBlockX(), MINY,
                        sectionPos.chunk().getMinBlockZ());
                int ORX = ocx * 16, ORZ = ocz * 16;
                TAG_ORX = ORX;
                TAG_ORZ = ORZ;

                Set<Holder<Biome>> possibleBiomes = new HashSet<>();
                ChunkPos.rangeClosed(sectionPos.chunk(), 1).forEach(chunkPos -> {
                    int qx = ChunkPos.getX(chunkPos.pack());
                    int qz = ChunkPos.getZ(chunkPos.pack());
                    ChunkAccess c = chunkAt(qx, qz);
                    for (var section : c.getSections()) {
                        section.getBiomes().getAll(possibleBiomes::add);
                    }
                });
                possibleBiomes.retainAll(biomeSource.possibleBiomes());

                WorldgenRandom random = new WorldgenRandom(
                        new XoroshiroRandomSource(RandomSupport.generateUniqueSeed()));
                long decorationSeed = random.setDecorationSeed(SEED,
                        ORX, ORZ);

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
                        random.setFeatureSeed(decorationSeed, gif, stepIndex);
                        // Trace sculk_vein/sculk_patch attempts for the center origin:
                        // replicate PlacedFeature.placeWithContext, logging every
                        // post-modifier position fed into Feature.place.
                        String tname = null;
                        if (ocx == CCX && ocz == CCZ && stepIndex == 7) {
                            String fn = String.valueOf(pf);
                            if (fn.contains("sculk_patch")) tname = "patch";
                            else if (fn.contains("sculk_vein")) tname = "vein";
                        }
                        try {
                            if (tname != null) {
                                System.out.println("TRACE " + tname + " start o=(" + ORX + "," + ORZ + ")");
                                var ctx = new net.minecraft.world.level.levelgen.placement.PlacementContext(
                                        (WorldGenLevel) levelProxy, generator, java.util.Optional.of(pf));
                                java.util.stream.Stream<BlockPos> placements =
                                        java.util.stream.Stream.of(origin);
                                for (net.minecraft.world.level.levelgen.placement.PlacementModifier pm
                                        : pf.placement()) {
                                    placements = placements.flatMap(
                                            p -> pm.getPositions(ctx, random, p));
                                }
                                ConfiguredFeature<?, ?> cf = pf.feature().value();
                                final String tn = tname;
                                org.apache.commons.lang3.mutable.MutableBoolean any =
                                        new org.apache.commons.lang3.mutable.MutableBoolean();
                                placements.forEach(bp -> {
                                    LOG.append("M|ATT|").append(tn).append('|')
                                       .append(bp.getX()).append('|').append(bp.getY())
                                       .append('|').append(bp.getZ()).append('\n');
                                    System.out.println("ATT " + tn + " "
                                            + bp.getX() + " " + bp.getY() + " " + bp.getZ());
                                    if (cf.place((WorldGenLevel) levelProxy, generator, random, bp)) {
                                        any.setTrue();
                                    }
                                    LOG.append("M|END|").append(tn).append('\n');
                                });
                            } else {
                                pf.placeWithBiomeCheck((WorldGenLevel) levelProxy,
                                        generator, random, origin);
                            }
                        } catch (Throwable t) {
                            System.out.println("ERROR placing " + gif + " origin ("
                                    + ocx + "," + ocz + "): " + t);
                        }
                    }
                }
            }
        }
        System.out.println("total_writes=" + WRITES);
        if (outPath != null) {
            try (var w = new java.io.PrintWriter(outPath, StandardCharsets.UTF_8)) {
                w.print(LOG);
            }
        }
    }

    @SuppressWarnings({"unchecked", "rawtypes"})
    static Object builtinRegistry(ResourceKey<?> key) {
        String path = key.identifier().getPath();
        switch (path) {
            case "block": return BuiltInRegistries.BLOCK;
            default: return null;
        }
    }
    static Object fakeRegistry() {
        Class<?>[] ifaces = new Class<?>[] {Registry.class};
        return Proxy.newProxyInstance(ProbeDecorate.class.getClassLoader(), ifaces,
                (p, m, a) -> {
                    switch (m.getName()) {
                        case "stream":
                            return java.util.stream.Stream.empty();
                        case "getResourceKey":
                            return java.util.Optional.empty();
                        case "iterator":
                            return java.util.Collections.emptyList().iterator();
                        case "keySet":
                            return java.util.Set.of();
                        case "size":
                            return 0;
                        default: {
                            Class<?> rt = m.getReturnType();
                            if (rt == boolean.class) return false;
                            if (rt.isPrimitive()) return 0;
                            if (java.util.Optional.class.isAssignableFrom(rt))
                                return java.util.Optional.empty();
                            return null;
                        }
                    }
                });
    }
    static Object regAccessStub(HolderLookup.Provider vanillaProvider) {
        return Proxy.newProxyInstance(
                ProbeDecorate.class.getClassLoader(),
                new Class<?>[]{RegistryAccess.class},
                (p, m, a) -> {
                    switch (m.getName()) {
                                                case "lookupOrThrow":
                        case "registryOrThrow": {
                            ResourceKey<?> key = (ResourceKey<?>) a[0];
                            String path = key.identifier().getPath();
                            if (path.equals("structure") || path.equals("placed_feature")) {
                                return fakeRegistry();
                            }
                            Object builtin = builtinRegistry(key);
                            if (builtin != null) return builtin;
                            return lkOrThrow(vanillaProvider, key);
                        }
                        case "lookup": {
                            ResourceKey<?> key = (ResourceKey<?>) a[0];
                            String path = key.identifier().getPath();
                            if (path.equals("structure") || path.equals("placed_feature")) {
                                return java.util.Optional.of(fakeRegistry());
                            }
                            Object builtin = builtinRegistry(key);
                            if (builtin != null)
                                return java.util.Optional.of(builtin);
                            return lk(vanillaProvider, key);
                        }case "toString":
                            return "OracleRegAccess";
                        default: {
                            Class<?> rt = m.getReturnType();
                            if (rt == boolean.class) return false;
                            if (rt.isPrimitive()) return 0;
                            throw new UnsupportedOperationException("RA." + m.getName());
                        }
                    }
                });
    }

    @SuppressWarnings({"unchecked", "rawtypes"})
    static Object lkOrThrow(Object provider, ResourceKey key) {
        return ((HolderLookup.Provider) provider).lookupOrThrow(key);
    }

    @SuppressWarnings({"unchecked", "rawtypes"})
    static Object lk(Object provider, ResourceKey key) {
        return ((HolderLookup.Provider) provider).lookup(key);
    }

    // ---------- level proxy handler ----------
    static BlockState getState(int x, int y, int z) {
        int lx = x - OX0, lz = z - OZ0;
        if (y < MINY || y >= TOP || lx < 0 || lx >= SIDE || lz < 0 || lz >= SIDE) {
            return Blocks.AIR.defaultBlockState();
        }
        return store[lz][y - MINY][lx];
    }

    static void setState(int x, int y, int z, BlockState st) {
        int lx = x - OX0, lz = z - OZ0;
        if (y < MINY || y >= TOP || lx < 0 || lx >= SIDE || lz < 0 || lz >= SIDE) {
            return;
        }
        store[lz][y - MINY][lx] = st;
    }

    // First non-proxy, non-probe stack frames = vanilla writer of this block.
    static String callerTag() {
        StringBuilder sb = new StringBuilder();
        int n = 0;
        for (StackTraceElement e : Thread.currentThread().getStackTrace()) {
            String cn = e.getClassName();
            if (cn.contains("ProbeDecorate") || cn.contains("Proxy")
                || cn.startsWith("java.") || cn.startsWith("jdk.")) {
                continue;
            }
            if (sb.length() > 0) sb.append("<-");
            sb.append(cn.substring(cn.lastIndexOf('.') + 1)).append(".").append(e.getMethodName());
            if (++n >= 9) break;
        }
        return sb.toString();
    }

    // ---- bind block tags from the bundled vanilla datapack ----
    //
    // A live server resolves #tags through TagLoader over its datapack; a bare
    // Bootstrap.bootStrap() does NOT, leaving every `state.is(TagKey)` false.
    // Without this, tag-gated worldgen (sculk attemptPlaceSculk conversions,
    // tree soil checks, ...) runs with dead gates and diverges from real
    // vanilla — the whole oracle comparison was poisoned by this.
    static void bindBlockTags() throws Exception {
        String jarPath = null;
        for (String p : System.getProperty("java.class.path")
                .split(java.io.File.pathSeparator)) {
            if (p.endsWith("server-26.2.jar")) { jarPath = p; break; }
        }
        if (jarPath == null) {
            throw new IllegalStateException("server-26.2.jar not on classpath");
        }
        java.util.zip.ZipFile zip = new java.util.zip.ZipFile(jarPath);
        net.minecraft.server.packs.PackResources pack =
                new net.minecraft.server.packs.PackResources() {
            public net.minecraft.server.packs.PackLocationInfo location() {
                return new net.minecraft.server.packs.PackLocationInfo(
                        "probe-vanilla", null, null, java.util.Optional.empty());
            }
            public net.minecraft.server.packs.resources.IoSupplier<java.io.InputStream>
                    getRootResource(String... path) { return null; }
            public net.minecraft.server.packs.resources.IoSupplier<java.io.InputStream>
                    getResource(net.minecraft.server.packs.PackType type,
                                net.minecraft.resources.Identifier id) {
                return entry(type.getDirectory() + "/" + id.getNamespace()
                        + "/" + id.getPath());
            }
            public void listResources(net.minecraft.server.packs.PackType type,
                                      String namespace, String directory,
                                      net.minecraft.server.packs.PackResources.ResourceOutput out) {
                String prefix = type.getDirectory() + "/" + namespace + "/"
                        + directory.replace('\\', '/') + "/";
                var en = zip.entries();
                while (en.hasMoreElements()) {
                    var e = en.nextElement();
                    String n = e.getName();
                    if (!n.startsWith(prefix) || !n.endsWith(".json")) continue;
                    String rest = n.substring("data/".length());
                    int slash = rest.indexOf('/');
                    var id = net.minecraft.resources.Identifier.fromNamespaceAndPath(
                            rest.substring(0, slash), rest.substring(slash + 1));
                    out.accept(id, entry(n));
                }
            }
            public java.util.Set<String> getNamespaces(net.minecraft.server.packs.PackType t) {
                return java.util.Set.of("minecraft");
            }
            public <T> T getMetadataSection(
                    net.minecraft.server.packs.metadata.MetadataSectionType<T> t) {
                return null;
            }
            public void close() {}
            private net.minecraft.server.packs.resources.IoSupplier<java.io.InputStream>
                    entry(String path) {
                var e = zip.getEntry(path);
                if (e == null) return null;
                return () -> zip.getInputStream(e);
            }
        };
        final net.minecraft.server.packs.PackResources packRef = pack;
        var rm = new net.minecraft.server.packs.resources.ResourceManager() {
            public java.util.Set<String> getNamespaces() { return java.util.Set.of(); }
            public java.util.Optional<net.minecraft.server.packs.resources.Resource> getResource(
                    net.minecraft.resources.Identifier id) {
                for (var t : net.minecraft.server.packs.PackType.values()) {
                    var sup = packRef.getResource(t, id);
                    if (sup != null) {
                        return java.util.Optional.of(
                                new net.minecraft.server.packs.resources.Resource(packRef, sup));
                    }
                }
                return java.util.Optional.empty();
            }
            public java.util.List<net.minecraft.server.packs.resources.Resource> getResourceStack(
                    net.minecraft.resources.Identifier id) {
                var r = getResource(id);
                return r.isEmpty() ? java.util.List.of() : java.util.List.of(r.get());
            }
            public java.util.Map<net.minecraft.resources.Identifier,
                    net.minecraft.server.packs.resources.Resource> listResources(
                    String directory, java.util.function.Predicate<net.minecraft.resources.Identifier> filter) {
                var out = new java.util.HashMap<net.minecraft.resources.Identifier,
                        net.minecraft.server.packs.resources.Resource>();
                for (var ns : new String[]{"minecraft"}) {
                    packRef.listResources(net.minecraft.server.packs.PackType.SERVER_DATA, ns,
                            directory,
                            (id, sup) -> {
                                if (filter.test(id)) {
                                    out.putIfAbsent(id, new net.minecraft.server.packs.resources.Resource(
                                            packRef, sup));
                                }
                            });
                }
                return out;
            }
            public java.util.Map<net.minecraft.resources.Identifier,
                    java.util.List<net.minecraft.server.packs.resources.Resource>> listResourceStacks(
                    String directory, java.util.function.Predicate<net.minecraft.resources.Identifier> filter) {
                var single = listResources(directory, filter);
                var out = new java.util.HashMap<net.minecraft.resources.Identifier,
                        java.util.List<net.minecraft.server.packs.resources.Resource>>();
                single.forEach((k, v) -> out.put(k, java.util.List.of(v)));
                return out;
            }
            public java.util.stream.Stream<net.minecraft.server.packs.PackResources> listPacks() {
                return java.util.stream.Stream.of(packRef);
            }
        };
        @SuppressWarnings({"unchecked", "rawtypes"})
        net.minecraft.tags.TagLoader.ElementLookup<net.minecraft.core.Holder<Block>> lookup =
                (net.minecraft.tags.TagLoader.ElementLookup)
                        TagLoader.ElementLookup.fromFrozenRegistry(BuiltInRegistries.BLOCK);
        java.util.Map<net.minecraft.tags.TagKey<Block>, java.util.List<net.minecraft.core.Holder<Block>>> tags =
                TagLoader.loadTagsForRegistry(rm, Registries.BLOCK, lookup);
        // Frozen registries reject bindTags(); vanilla's own reload path is
        // PendingTags.apply() (binds named sets + refreshes holder tags).
        var result = new TagLoader.LoadResult<>(Registries.BLOCK, tags);
        ((net.minecraft.core.WritableRegistry<Block>) BuiltInRegistries.BLOCK)
                .prepareTagReload(result).apply();
    }

    static Object handle(String name, Class<?> ret, Object[] a) {        boolean hasPos = a != null && a.length > 0 && a[0] instanceof BlockPos;
        BlockPos pos = hasPos ? (BlockPos) a[0] : null;

        switch (name) {
            case "getBlockState": {
                if (pos != null && pos.getX() == 98 && pos.getY() == -44 && pos.getZ() == -23
                        && System.getenv("PROBE_WATCH") != null) {
                    int c = WATCH_COUNT++;
                    if (c < 120) {
                        StackTraceElement[] st = Thread.currentThread().getStackTrace();
                        StringBuilder sb = new StringBuilder();
                        int n = 0;
                        for (StackTraceElement e : st) {
                            String cn = e.getClassName();
                            if (cn.contains("ProbeDecorate") || cn.contains("Proxy")
                                    || cn.startsWith("java.") || cn.startsWith("jdk.")) continue;
                            if (sb.length() > 0) sb.append("<-");
                            sb.append(cn.substring(cn.lastIndexOf('.') + 1)).append('.').append(e.getMethodName());
                            if (++n >= 6) break;
                        }
                        System.out.println("WATCH[" + c + "] get (98,-44,-23) by " + sb);
                    }
                }
                return getState(pos.getX(), pos.getY(), pos.getZ());
            }
            case "setBlock": {
                BlockState st = (BlockState) a[1];
                setState(pos.getX(), pos.getY(), pos.getZ(), st);
                WRITES++;
                String tag = TAG_ORX + "|" + TAG_ORZ;
                if (st.is(Blocks.SCULK_VEIN)) {
                    tag = TAG_ORX + "|" + TAG_ORZ + "|" + callerTag();
                }
                LOG.append(pos.getX()).append('|').append(pos.getY()).append('|')
                   .append(pos.getZ()).append('|')
                   .append(BuiltInRegistries.BLOCK.getKey(st.getBlock())).append('|')
                   .append(tag).append('\n');
                return true;
            }
            case "removeBlock":
            case "destroyBlock": {
                setState(pos.getX(), pos.getY(), pos.getZ(),
                        Blocks.AIR.defaultBlockState());
                WRITES++;
                LOG.append(pos.getX()).append('|').append(pos.getY()).append('|')
                   .append(pos.getZ()).append("|minecraft:air|")
                   .append(TAG_ORX).append('|').append(TAG_ORZ).append('\n');
                return true;
            }
            case "getFluidState":
                return getState(pos.getX(), pos.getY(), pos.getZ()).getFluidState();
            case "getBlockEntity":
                return null;
            case "getHeight": {
                if (a == null || a.length < 3) return HEIGHT; // gen-depth form
                String type = a[0].toString();
                return scanHeight(type, (Integer) a[1], (Integer) a[2]);
            }
            case "getGenDepth":
                return HEIGHT;
            case "getMinY":
            case "getMinBuildHeight":
                return MINY;
            case "getMaxY":
            case "getMaxBuildHeight":
                return TOP;
            case "getMinSectionY":
                return MINY >> 4;
            case "getMaxSectionY":
                return (TOP >> 4) - 1;
            case "getSectionsCount":
                return HEIGHT / 16;
            case "getSectionIndex":
                return ((Integer) a[0] >> 4) - (MINY >> 4);
            case "getSectionIndexFromSectionY":
                return (Integer) a[0] - (MINY >> 4);
            case "isOutsideBuildHeight":
                if (a[0] instanceof Integer yy) return yy < MINY || yy >= TOP;
                if (a[0] instanceof BlockPos bp)
                    return bp.getY() < MINY || bp.getY() >= TOP;
                return true;
            case "getChunk": {
                int cx, cz;
                if (a[0] instanceof BlockPos bp) {
                    cx = bp.getX() >> 4; cz = bp.getZ() >> 4;
                } else {
                    cx = (Integer) a[0]; cz = (Integer) a[1];
                }
                try {
                    return chunkAt(cx, cz);
                } catch (Throwable t) {
                    System.out.println("getChunk(" + cx + "," + cz + ") FAILED");
                    new Throwable().printStackTrace(System.out);
                    throw t;
                }
            }
            case "hasChunkAt":
            case "isLoaded":
            case "hasReferences":
            case "ensureCanWrite":
                return true;
            case "registryAccess":
                return REG_ACCESS;
            case "getBiome":
                return BIOME_MGR.getBiome(pos);
            case "getBiomeManager":
                return BIOME_MGR;
            case "getLightEngine":
                return LIGHT;
            case "dimension":
                return Level.OVERWORLD;
            case "dimensionType":
                return DIM_TYPE;
            case "getSeaLevel":
                return 63;
            case "getSeed":
                return SEED;
            case "isEmptyBlock":
                return pos != null && getState(pos.getX(), pos.getY(), pos.getZ()).isAir();
            case "getDifficulty":
                return net.minecraft.world.Difficulty.NORMAL;
            case "getSharedSpawnPos":
                return new BlockPos(OX0 + SIDE / 2, 64, OZ0 + SIDE / 2);
            case "getSharedSpawnAngle":
                return 0.0f;
            case "random":
                return new java.util.Random(0);
            case "playSound":
            case "levelEvent":
            case "addParticle":
            case "gameEvent":
            case "scheduleTick":
            case "scheduleFluidTick":
            case "scheduleBlockTick":
            case "getEntities":
            case "getNearestPlayer":
            case "players":
                return null;
            default: {
                if (ret == boolean.class) return false;
                if (ret.isPrimitive()) return 0;
                throw new UnsupportedOperationException("OracleLevel." + name);
            }
        }
    }

    static int scanHeight(String type, int x, int z) {
        boolean oceanFloor = type.contains("OCEAN");
        boolean worldSurface = type.contains("WORLD_SURFACE");
        for (int y = TOP - 1; y > MINY; y--) {
            BlockState s = getState(x, y, z);
            boolean ok;
            if (oceanFloor) {
                ok = s.blocksMotion() || !s.getFluidState().isEmpty();
            } else if (worldSurface) {
                ok = !s.isAir();
            } else {
                ok = s.blocksMotion() || !s.getFluidState().isEmpty();
            }
            if (ok) return y + 1;
        }
        return MINY;
    }

    static ChunkAccess chunkAt(int cx, int cz) {
        int dx = cx - CCX + RADIUS;
        int dz = cz - CCZ + RADIUS;
        if (dx < 0 || dz < 0 || dx >= N || dz >= N) {
            throw new IllegalStateException("chunk out of oracle range (" + cx + "," + cz + ")");
        }
        return CHUNKS[dz][dx];
    }

    static void loadDump(String path) throws Exception {
        try (DataInputStream in = new DataInputStream(new FileInputStream(path))) {
            byte[] magic = new byte[5];
            in.readFully(magic);
            if (!new String(magic, StandardCharsets.US_ASCII).equals("NDEC1")) {
                throw new IllegalStateException("bad magic");
            }
            SEED = i64LE(in);
            CCX = i32LE(in);
            CCZ = i32LE(in);
            OX0 = (CCX - RADIUS) * 16;
            OZ0 = (CCZ - RADIUS) * 16;
            int bioTable = u16LE(in);
            BIOME_NAMES.clear();
            for (int i = 0; i < bioTable; i++) {
                int len = u16LE(in);
                byte[] b = new byte[len];
                in.readFully(b);
                BIOME_NAMES.add(new String(b, StandardCharsets.UTF_8));
            }
            store = new BlockState[SIDE][HEIGHT][SIDE];
            BlockState air = Blocks.AIR.defaultBlockState();
            for (int z = 0; z < SIDE; z++) {
                for (int y = 0; y < HEIGHT; y++) {
                    java.util.Arrays.fill(store[z][y], air);
                }
            }
            QUART = new byte[N][N][];
            for (int cz = 0; cz < N; cz++) {
                for (int cx = 0; cx < N; cx++) {
                    int palCount = u16LE(in);
                    List<BlockState> pal = new ArrayList<>(palCount);
                    for (int i = 0; i < palCount; i++) {
                        int len = u16LE(in);
                        byte[] b = new byte[len];
                        in.readFully(b);
                        String nm = new String(b, StandardCharsets.UTF_8);
                        Block blk = BuiltInRegistries.BLOCK.getOptional(Identifier.parse(nm))
                                .orElseThrow(() -> new IllegalStateException("block " + nm));
                        pal.add(blk.defaultBlockState());
                    }
                    int wx0 = OX0 + cx * 16, wz0 = OZ0 + cz * 16;
                    for (int y = MINY; y < TOP; y++) {
                        int yy = y - MINY;
                        for (int lz = 0; lz < 16; lz++) {
                            for (int lx = 0; lx < 16; lx++) {
                                int src = yy * 256 + lz * 16 + lx;
                                store[wz0 - OZ0 + lz][yy][wx0 - OX0 + lx] =
                                        pal.get(u16LE(in));
                            }
                        }
                    }
                    QUART[cz][cx] = new byte[1536];
                    in.readFully(QUART[cz][cx]);
                }
            }
            System.out.println("dump loaded: center=(" + CCX + "," + CCZ + ")"
                    + " biomes=" + BIOME_NAMES.size());
        }
        // Tag-binding sanity check: a live server resolves block tags from the
        // bundled datapack; a bare bootstrap does NOT. If this prints false,
        // every `state.is(TagKey)` gate in this probe is dead and the oracle
        // diverges from real vanilla on tag-dependent paths.
        System.out.println("TAGCHECK deepslate∈sculk_replaceable_world_gen = "
                + Blocks.DEEPSLATE.defaultBlockState()
                        .is(net.minecraft.tags.BlockTags.SCULK_REPLACEABLE_WORLD_GEN));
    }

    /**
     * Manual PalettedContainerFactory: same as vanilla create() but the biome
     * IdMap is built from the datapack lookup's holders (no Registry needed).
     * Codecs are null — chunks are never serialized in this oracle.
     */
    static PalettedContainerFactory palettedContainerFactoryManual(
            HolderLookup.Provider lookup) {
        var biomeReg = lookup.lookupOrThrow(Registries.BIOME);
        List<Holder<Biome>> all = new ArrayList<>();
        biomeReg.listElements().forEach(h -> all.add(h));
        java.util.HashMap<Holder<Biome>, Integer> ids = new java.util.HashMap<>();
        net.minecraft.core.IdMap<Holder<Biome>> idMap =
                new net.minecraft.core.IdMap<>() {
                    @Override public int getId(Holder<Biome> h) {
                        Integer i = ids.get(h);
                        return i == null ? -1 : i;
                    }
                    @Override public Holder<Biome> byId(int i) {
                        return i >= 0 && i < all.size() ? all.get(i) : null;
                    }
                    @Override public int size() { return all.size(); }
                    @Override public java.util.Iterator<Holder<Biome>> iterator() {
                        return all.iterator();
                    }
                };
        var blockStrategy = Strategy.createForBlockStates(Block.BLOCK_STATE_REGISTRY);
        var biomeStrategy = Strategy.createForBiomes(idMap);
        Holder<Biome> plains = biomeReg.getOrThrow(ResourceKey.create(
                Registries.BIOME, Identifier.parse("minecraft:plains")));
        return new PalettedContainerFactory(
                blockStrategy,
                Blocks.AIR.defaultBlockState(),
                null,
                biomeStrategy,
                plains,
                null);
    }

    static int u16LE(DataInputStream in) throws java.io.IOException {
        int b1 = in.readUnsignedByte();
        int b2 = in.readUnsignedByte();
        return (b2 << 8) | b1;
    }

    static int i32LE(DataInputStream in) throws java.io.IOException {
        return (u16LE(in) & 0xFFFF) | (u16LE(in) << 16);
    }

    static long i64LE(DataInputStream in) throws java.io.IOException {
        long lo = i32LE(in) & 0xFFFFFFFFL;
        long hi = i32LE(in) & 0xFFFFFFFFL;
        return (hi << 32) | lo;
    }
}
