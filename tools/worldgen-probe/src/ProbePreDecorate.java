import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.lang.reflect.Proxy;
import java.util.ArrayList;
import java.util.List;
import net.minecraft.SharedConstants;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Holder;
import net.minecraft.core.HolderGetter;
import net.minecraft.core.HolderLookup;
import net.minecraft.core.registries.Registries;
import net.minecraft.data.registries.VanillaRegistries;
import net.minecraft.server.Bootstrap;
import net.minecraft.world.level.ChunkPos;
import net.minecraft.world.level.LevelHeightAccessor;
import net.minecraft.world.level.biome.Biome;
import net.minecraft.world.level.biome.BiomeManager;
import net.minecraft.world.level.biome.BiomeSource;
import net.minecraft.world.level.biome.Biomes;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.chunk.CarvingMask;
import net.minecraft.world.level.chunk.ChunkAccess;
import net.minecraft.world.level.chunk.LevelChunkSection;
import net.minecraft.world.level.chunk.ProtoChunk;
import net.minecraft.world.ticks.ProtoChunkTicks;
import net.minecraft.world.level.chunk.UpgradeData;
import net.minecraft.world.level.levelgen.LegacyRandomSource;
import net.minecraft.world.level.levelgen.NoiseBasedChunkGenerator;
import net.minecraft.world.level.levelgen.NoiseChunk;
import net.minecraft.world.level.levelgen.NoiseGeneratorSettings;
import net.minecraft.core.QuartPos;
import net.minecraft.world.level.levelgen.RandomState;
import net.minecraft.world.level.levelgen.WorldgenRandom;
import net.minecraft.world.level.levelgen.blending.Blender;
import net.minecraft.world.level.levelgen.carver.CarvingContext;
import net.minecraft.world.level.levelgen.carver.ConfiguredWorldCarver;
import net.minecraft.world.level.levelgen.carver.WorldCarver;
import net.minecraft.world.level.levelgen.synth.NormalNoise;
import net.minecraft.world.ticks.ProtoChunkTicks;

/**
 * Dumps the REAL vanilla pre-decoration state (fillFromNoise + buildSurface +
 * applyCarvers, NO features) for a 5x5 chunk window.
 *
 * Output (little-endian): "PREDC1" | seed:i64 | ccx:i32 | ccz:i32 | then 25
 * chunks in dz-outer/dx-inner order: u16 paletteCount, entries (u16 len +
 * utf8), 98304 u16 indices ((y+64)*256 + z*16 + x, 0 = "minecraft:air").
 *
 * Usage: ProbePreDecorate <seed> <ccx> <ccz> <outFile>
 */
public class ProbePreDecorate {
    static final int MINY = -64, TOP = 320, HEIGHT = TOP - MINY;
    static final int R = 2, N = 5;
    static final String MAGIC = "PREDC1";

    static long SEED;
    static int CCX, CCZ;
    static ProtoChunk[][] CHUNKS;
    static NoiseBasedChunkGenerator GEN;
    static RandomState RS;
    static Holder<Biome> PLAINS;
    static LevelHeightAccessor LHA;

    public static void main(String[] args) throws Exception {
        SEED = Long.parseLong(args[0]);
        CCX = Integer.parseInt(args[1]);
        CCZ = Integer.parseInt(args[2]);
        String out = args[3];
        SharedConstants.tryDetectVersion();
        Bootstrap.bootStrap();
        HolderLookup.Provider lookup = VanillaRegistries.createLookup();
        HolderGetter<NormalNoise.NoiseParameters> noises = lookup.lookupOrThrow(Registries.NOISE);
        Holder<NoiseGeneratorSettings> settings =
                lookup.lookupOrThrow(Registries.NOISE_SETTINGS).getOrThrow(NoiseGeneratorSettings.OVERWORLD);
        RS = RandomState.create(settings.value(), noises, SEED);
        PLAINS = lookup.lookupOrThrow(Registries.BIOME).getOrThrow(Biomes.PLAINS);
        GEN = new NoiseBasedChunkGenerator(new net.minecraft.world.level.biome.FixedBiomeSource(PLAINS), settings);
        var factory = ProbeDecorate.palettedContainerFactoryManual(lookup);
        // structureManager is NOT used: the NoiseChunk (with a neutral
        // BeardifierMarker) is pre-attached via reflection below.
        net.minecraft.world.level.StructureManager structureManager = null;
        var regAccess = (net.minecraft.core.RegistryAccess) ProbeDecorate.regAccessStub(lookup);

        LHA = new LevelHeightAccessor() {
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

        // ---- Step 1: empty ProtoChunks with REAL noise-biome sections ----
        // NOTE: FixedBiomeSource = plains everywhere. Surface-rule biome
        // conditions would all resolve to plains — fine for geometry/surface
        // mechanics comparison, documented limitation for biome-specific
        // surface rules.
        CHUNKS = new ProtoChunk[N][N];
        for (int cz = 0; cz < N; cz++) {
            for (int cx = 0; cx < N; cx++) {
                int ccx = CCX - R + cx, ccz = CCZ - R + cz;
                LevelChunkSection[] secs = new LevelChunkSection[HEIGHT / 16];
                for (int s = 0; s < secs.length; s++) {
                    var biomesC = factory.createForBiomes();
                    for (int sy = 0; sy < 4; sy++)
                        for (int bz4 = 0; bz4 < 4; bz4++)
                            for (int bx4 = 0; bx4 < 4; bx4++)
                                biomesC.set(bx4, sy, bz4, PLAINS);
                    secs[s] = new LevelChunkSection(factory.createForBlockStates(), biomesC);
                }
                CHUNKS[cz][cx] = new ProtoChunk(new ChunkPos(ccx, ccz), UpgradeData.EMPTY, secs,
                        new ProtoChunkTicks<>(), new ProtoChunkTicks<>(), LHA, factory, null);
            }
        }

        // ---- Step 2: fillFromNoise (real density, beardifier empty) ----
        Field fpField = NoiseBasedChunkGenerator.class.getDeclaredField("globalFluidPicker");
        fpField.setAccessible(true);
        var fluid = (net.minecraft.world.level.levelgen.Aquifer.FluidPicker)
                ((java.util.function.Supplier<?>) fpField.get(GEN)).get();
        var beardCls = Class.forName("net.minecraft.world.level.levelgen.DensityFunctions$BeardifierMarker");
        var bf = beardCls.getField("INSTANCE");
        bf.setAccessible(true);
        var beard = (net.minecraft.world.level.levelgen.DensityFunctions.BeardifierOrMarker) bf.get(null);
        Field ncField = ChunkAccess.class.getDeclaredField("noiseChunk");
        ncField.setAccessible(true);
        var nsClamped = settings.value().noiseSettings().clampToHeightAccessor(LHA);
        for (int cz = 0; cz < N; cz++) {
            for (int cx = 0; cx < N; cx++) {
                var nc = NoiseChunk.forChunk(CHUNKS[cz][cx], RS, beard, settings.value(), fluid, Blender.empty());
                ncField.set(CHUNKS[cz][cx], nc);
            }
        }
        for (int cz = 0; cz < N; cz++) {
            for (int cx = 0; cx < N; cx++) {
                ChunkAccess done = GEN.fillFromNoise(Blender.empty(), RS, structureManager,
                        CHUNKS[cz][cx]).join();
                CHUNKS[cz][cx] = (ProtoChunk) done;
            }
        }
        System.out.println("fillFromNoise done");

        BiomeManager biomeManager = new BiomeManager(
                (BiomeManager.NoiseBiomeSource) GEN.getBiomeSource(),
                BiomeManager.obfuscateSeed(SEED));

        // ---- Step 3: buildSurface per INNER chunk (real rule application) ----
        // Direct private overload: buildSurface(chunk, context, rs, sm, bm, blender, possibleBiomes)
        Method bs = null;
        for (Method m : NoiseBasedChunkGenerator.class.getMethods()) {
            if (m.getName().equals("buildSurface") && m.getParameterCount() == 7) {
                bs = m; bs.setAccessible(true); break;
            }
        }
        var wgc = new net.minecraft.world.level.levelgen.WorldGenerationContext(GEN, LHA);
        var possibleBiomes = new java.util.HashSet<Holder<Biome>>();
        possibleBiomes.add(PLAINS);
        for (int cz = 1; cz < N - 1; cz++) {
            for (int cx = 1; cx < N - 1; cx++) {
                bs.invoke(GEN, CHUNKS[cz][cx], wgc, RS, structureManager, biomeManager,
                        Blender.empty(), possibleBiomes);
            }
        }
        System.out.println("buildSurface done");

        // ---- Step 4: applyCarvers driver (dx outer, dz inner) ----
        var carverReg = lookup.lookupOrThrow(Registries.CONFIGURED_CARVER);
        ConfiguredWorldCarver<?> caveC =
                carverReg.getOrThrow(net.minecraft.data.worldgen.Carvers.CAVE).value();
        ConfiguredWorldCarver<?> caveExtraC =
                carverReg.getOrThrow(net.minecraft.data.worldgen.Carvers.CAVE_EXTRA_UNDERGROUND).value();
        ConfiguredWorldCarver<?> canyonC =
                carverReg.getOrThrow(net.minecraft.data.worldgen.Carvers.CANYON).value();

        BiomeSource source = GEN.getBiomeSource();

        for (int tzl = 1; tzl < N - 1; tzl++) {
            for (int txl = 1; txl < N - 1; txl++) {
                int tcx = CCX - R + txl, tcz = CCZ - R + tzl;
                ChunkAccess target = CHUNKS[tzl][txl];
                NoiseChunk noiseChunk = target.getOrCreateNoiseChunk(c -> {
                    throw new IllegalStateException("noise chunk should be cached by fillFromNoise");
                });
                CarvingContext context = new CarvingContext(GEN, regAccess, LHA,
                        noiseChunk, RS, settings.value().surfaceRule());
                var mask = new CarvingMask(HEIGHT, MINY);
                var aquifer = noiseChunk.aquifer();
                for (int dx = -8; dx <= 8; dx++) {
                    for (int dz = -8; dz <= 8; dz++) {
                        int scx = tcx + dx, scz = tcz + dz;
                        if (Math.abs(scx - CCX) > R || Math.abs(scz - CCZ) > R) {
                            continue; // source not materialized in this probe
                        }
                        var biomeGetter = (java.util.function.Function<BlockPos, Holder<Biome>>)
                                p -> PLAINS;
                        WorldgenRandom random =
                                new WorldgenRandom(new LegacyRandomSource(0L));
                        int index = 0;
                        // vanilla overworld carver list order
                        for (ConfiguredWorldCarver<?> cc : new ConfiguredWorldCarver<?>[]{caveC, caveExtraC, canyonC}) {
                            random.setLargeFeatureSeed(SEED + index, scx, scz);
                            if (cc.isStartChunk(random)) {
                                cc.carve(context, target, biomeGetter, random,
                                        aquifer, new ChunkPos(scx, scz), mask);
                            }
                            index++;
                        }
                        var _keep = source; // biome-dependent lists differ only for ocean
                    }
                }
            }
        }
        System.out.println("applyCarvers done");

        // ---- Step 5: dump ----
        try (var os = new java.io.DataOutputStream(new java.io.FileOutputStream(out))) {
            os.write(MAGIC.getBytes("US-ASCII"));
            os.writeLong(SEED);
            os.writeInt(CCX);
            os.writeInt(CCZ);
            for (int cz = 0; cz < N; cz++) {
                for (int cx = 0; cx < N; cx++) {
                    ProtoChunk ch = CHUNKS[cz][cx];
                    List<String> pal = new ArrayList<>();
                    pal.add("minecraft:air");
                    List<Integer> idxs = new ArrayList<>(16 * HEIGHT * 16);
                    for (int s = 0; s < ch.getSectionsCount(); s++) {
                        LevelChunkSection sec = ch.getSection(s);
                        for (int ly = 0; ly < 16; ly++) {
                            for (int lz = 0; lz < 16; lz++) {
                                for (int lx = 0; lx < 16; lx++) {
                                    BlockState st = sec.getBlockState(lx, ly, lz);
                                    String n = st.isAir() ? "minecraft:air"
                                            : net.minecraft.core.registries.BuiltInRegistries.BLOCK
                                                    .getKey(st.getBlock()).toString();
                                    int pi = pal.indexOf(n);
                                    if (pi < 0) { pi = pal.size(); pal.add(n); }
                                    idxs.add(pi);
                                }
                            }
                        }
                    }
                    os.writeShort(pal.size());
                    for (String nm : pal) {
                        byte[] b = nm.getBytes("UTF-8");
                        os.writeShort(b.length);
                        os.write(b);
                    }
                    for (Integer pi : idxs) os.writeShort(pi);
                }
            }
        }
        System.out.println("dumped -> " + out);
    }

    static Object handleRegion(String name, Class<?> ret, Object[] a) {
        switch (name) {
            case "getCenter":
                return new ChunkPos(CCX, CCZ);
            case "getSeed":
                return SEED;
            case "getChunk": {
                int cx, cz;
                if (a[0] instanceof BlockPos bp) {
                    cx = bp.getX() >> 4; cz = bp.getZ() >> 4;
                } else {
                    cx = (Integer) a[0]; cz = (Integer) a[1];
                }
                int dx = cx - (CCX - R), dz = cz - (CCZ - R);
                if (dx < 0 || dz < 0 || dx >= N || dz >= N) {
                    throw new IllegalStateException("getChunk out of probe range (" + cx + "," + cz + ")");
                }
                return CHUNKS[dz][dx];
            }
            case "getBlender":
                return Blender.empty();
            case "hasChunkAt":
            case "isLoaded":
            case "hasReferences":
            case "ensureCanWrite":
                return true;
            case "getMinBuildHeight":
                return MINY;
            case "getMaxBuildHeight":
            case "getHeight":
                return TOP;
            case "getSectionsCount":
                return HEIGHT / 16;
            case "registryAccess":
                return ProbeDecorate.REG_ACCESS;
            case "getBiomeManager":
                return new BiomeManager(
                        (BiomeManager.NoiseBiomeSource) GEN.getBiomeSource(),
                        BiomeManager.obfuscateSeed(SEED));
            case "getSeaLevel":
                return 63;
            default:
                Class<?> r = ret;
                if (r == int.class) return 0;
                if (r == boolean.class) return false;
                if (r == List.class) return new ArrayList<>();
                if (r == java.util.Optional.class) return java.util.Optional.empty();
                return null;
        }
    }
}
