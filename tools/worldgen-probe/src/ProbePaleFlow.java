import java.lang.reflect.Proxy;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.HashMap;
import java.util.Map;
import java.util.Optional;
import net.minecraft.SharedConstants;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Direction;
import net.minecraft.core.Holder;
import net.minecraft.core.RegistryAccess;
import net.minecraft.core.registries.Registries;
import net.minecraft.resources.Identifier;
import net.minecraft.resources.ResourceKey;
import net.minecraft.server.Bootstrap;
import net.minecraft.world.level.LevelAccessor;
import net.minecraft.world.level.WorldGenLevel;
import net.minecraft.world.level.biome.Biome;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.Blocks;
import net.minecraft.world.level.block.LeavesBlock;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.chunk.ChunkAccess;
import net.minecraft.world.level.chunk.ChunkGenerator;
import net.minecraft.world.level.chunk.status.ChunkStatus;
import net.minecraft.world.level.levelgen.Heightmap;
import net.minecraft.world.level.levelgen.WorldgenRandom;
import net.minecraft.world.level.levelgen.XoroshiroRandomSource;
import net.minecraft.world.level.levelgen.placement.PlacedFeature;
import net.minecraft.world.level.material.FluidState;
import net.minecraft.world.level.material.Fluids;

/**
 * Full vanilla pale_garden_vegetation placement flow for chunk (0,0) seed
 * 424242, using the REAL PlacedFeature chain (count/in_square/water_depth/
 * heightmap/biome -> random_selector -> pale_oak_checked -> would_survive
 * -> TreeFeature + decorators). Terrain = vanilla-fresh-424242 minus trees.
 *
 * java ProbePaleFlow [terrain.txt] [seed] [cx] [cz] [featureIndex]
 */
public class ProbePaleFlow {
    static int nextBits = 0; // underlying WorldgenRandom.next(bits) calls

    public static void main(String[] args) throws Exception {
        SharedConstants.tryDetectVersion();
        Bootstrap.bootStrap();
        bindSupportsVegetationTags();

        String terrainPath = args.length > 0 ? args[0] : "tmp-vanilla-terrain-3x3.txt";
        long seed = args.length > 1 ? Long.parseLong(args[1]) : 424242L;
        int cx = args.length > 2 ? Integer.parseInt(args[2]) : 0;
        int cz = args.length > 3 ? Integer.parseInt(args[3]) : 0;
        int featureIndex = args.length > 4 ? Integer.parseInt(args[4]) : 13;

        Map<BlockPos, BlockState> blocks = new HashMap<>();
        for (String line : Files.readAllLines(Path.of(terrainPath))) {
            line = line.trim();
            if (line.isEmpty()) continue;
            String[] q = line.split("\\s+");
            int x = Integer.parseInt(q[0]);
            int y = Integer.parseInt(q[1]);
            int z = Integer.parseInt(q[2]);
            String name = q[3];
            Block blk = net.minecraft.core.registries.BuiltInRegistries.BLOCK
                    .get(Identifier.parse(name))
                    .map(Holder.Reference::value)
                    .orElse(Blocks.AIR);
            blocks.put(new BlockPos(x, y, z).immutable(), blk.defaultBlockState());
        }
        System.out.println("loaded terrain cells=" + blocks.size());

        net.minecraft.core.HolderLookup.Provider lookup = net.minecraft.data.registries.VanillaRegistries.createLookup();
        RegistryAccess registry = registryAccessFrom(lookup);
        Holder<Biome> paleGarden = lookup.lookupOrThrow(Registries.BIOME)
                .getOrThrow(ResourceKey.create(Registries.BIOME,
                        Identifier.parse("minecraft:pale_garden")));
        PlacedFeature placed = lookup.lookupOrThrow(Registries.PLACED_FEATURE)
                .getOrThrow(ResourceKey.create(Registries.PLACED_FEATURE,
                        Identifier.parse("minecraft:pale_garden_vegetation"))).value();

        ChunkAccess chunk = dummyChunk();
        ChunkGenerator generator = fakeGenerator(registry, paleGarden);
        dummyGenerator = generator;
        serverLevelStub = makeServerLevel(generator);
        WorldGenLevel level = fakeLevel(blocks, chunk, registry, paleGarden);

        WorldgenRandom rng = new WorldgenRandom(new XoroshiroRandomSource(seed)) {
            @Override public int next(int bits) {
                nextBits++;
                int v = super.next(bits);
                if (System.getenv("PALE_RAW") != null) {
                    System.out.println("RAW next(" + bits + ")=" + v + " bits=" + nextBits);
                }
                return v;
            }
            @Override public int nextInt(int bound) {
                int v = super.nextInt(bound);
                if (System.getenv("PALE_TRACE") != null) {
                    System.out.println("RNG nextInt(" + bound + ")=" + v + " bits=" + nextBits);
                }
                return v;
            }
            @Override public float nextFloat() {
                float v = super.nextFloat();
                if (System.getenv("PALE_TRACE") != null) {
                    System.out.println("RNG nextFloat=" + v + " bits=" + nextBits);
                }
                return v;
            }
            @Override public boolean nextBoolean() {
                boolean v = super.nextBoolean();
                if (System.getenv("PALE_TRACE") != null) {
                    System.out.println("RNG nextBoolean=" + v + " bits=" + nextBits);
                }
                return v;
            }
        };
        long dec = rng.setDecorationSeed(seed, cx * 16, cz * 16);
        System.out.println("dec=" + dec + " idx=" + featureIndex);
        rng.setFeatureSeed(dec, featureIndex, 9);
        int before = nextBits;
        // Manual modifier chain (count 16 -> in_square -> water_depth ->
        // heightmap -> biome -> feature) so each draw/filter can be logged.
        boolean placedAny = false;
        for (int d = 0; d < 16; d++) {
            int x = cx * 16 + rng.nextInt(16);
            int z = cz * 16 + rng.nextInt(16);
            // surface_water_depth_filter
            int yOcean = level.getHeight(Heightmap.Types.OCEAN_FLOOR, x, z);
            int ySurf = level.getHeight(Heightmap.Types.WORLD_SURFACE, x, z);
            boolean waterOk = ySurf - yOcean <= 0;
            // heightmap OCEAN_FLOOR_WG
            int y = level.getHeight(Heightmap.Types.OCEAN_FLOOR_WG, x, z);
            // biome filter
            Holder<Biome> biome = level.getBiome(new BlockPos(x, y, z));
            boolean biomeOk = generator.getBiomeGenerationSettings(biome).hasFeature(placed);
            boolean wouldSurvive = false;
            if (biomeOk) {
                BlockState below = level.getBlockState(new BlockPos(x, y - 1, z));
                wouldSurvive = wouldSurviveCheck(below);
            }
            System.out.println("draw " + (d + 1) + " pos=(" + x + "," + z + ") y=" + y
                    + " water=" + waterOk + " biome=" + biomeOk + " below="
                    + belowName(level, x, y - 1, z) + " surv=" + wouldSurvive
                    + " bits=" + nextBits);
            // PALE_SKIP=N: reject draws 1..N after gates (RNG parity with the
            // rust NEUTRON_DECO_SKIP_TREE_DRAWS diagnostic — pos+gates consumed,
            // no dispatch).
            int skipN = Integer.getInteger("pale.skip", 0);
            if (d + 1 <= skipN) {
                System.out.println("draw " + (d + 1) + " SKIP");
                continue;
            }
            if (!(waterOk && biomeOk && wouldSurvive)) {
                continue;
            }
            int preLogs = countBlock(blocks, Blocks.PALE_OAK_LOG);
            int preLeaves = countBlock(blocks, Blocks.PALE_OAK_LEAVES);
            // RandomSelectorFeature: 0.1 creaking / 0.9 checked, default checked.
            int beforeDraw = nextBits;
            float f1 = rng.nextFloat();
            float f2 = 0f;
            boolean creaking = f1 < 0.1f;
            if (!creaking) {
                f2 = rng.nextFloat();
            }
            String chosen = creaking ? "pale_oak_creaking_checked" : "pale_oak_checked";
            net.minecraft.core.Holder<PlacedFeature> chosenHolder = lookup
                    .lookupOrThrow(Registries.PLACED_FEATURE)
                    .getOrThrow(ResourceKey.create(Registries.PLACED_FEATURE,
                            Identifier.parse("minecraft:" + chosen)));
            boolean ok = false;
            // Manual inner flow: block_predicate_filter then feature.place.
            try {
                net.minecraft.world.level.levelgen.placement.BlockPredicateFilter bpf =
                        (net.minecraft.world.level.levelgen.placement.BlockPredicateFilter)
                                chosenHolder.value().placement().get(0);
                var pf = bpf.getClass().getDeclaredField("predicate");
                pf.setAccessible(true);
                Object pred = pf.get(bpf);
                var m = pred.getClass().getDeclaredMethod("test",
                        net.minecraft.world.level.WorldGenLevel.class,
                        net.minecraft.core.BlockPos.class);
                m.setAccessible(true);
                boolean predOk = (Boolean) m.invoke(pred, level, new BlockPos(x, y, z));
                System.out.println("draw " + (d + 1) + " inner pred=" + predOk);
                if (predOk) {
                    net.minecraft.world.level.levelgen.feature.ConfiguredFeature<?, ?> cf =
                            chosenHolder.value().feature().value();
                    System.out.println("draw " + (d + 1) + " feature class="
                            + cf.feature().getClass().getSimpleName());
                    ok = cf.place(level, generator, rng, new BlockPos(x, y, z));
                }
            } catch (Throwable t) {
                System.out.println("draw " + (d + 1) + " THREW " + t);
                t.printStackTrace(System.out);
                throw t;
            }
            System.out.println("draw " + (d + 1) + " PLACED chosen=" + chosen + " f1=" + f1
                    + " f2=" + f2 + " ok=" + ok + " consumed=" + (nextBits - beforeDraw)
                    + " deltaLogs=" + (countBlock(blocks, Blocks.PALE_OAK_LOG) - preLogs)
                    + " deltaLeaves=" + (countBlock(blocks, Blocks.PALE_OAK_LEAVES) - preLeaves));
            if (System.getenv("PALE_DUMPALL") != null && d == 15) {
                StringBuilder sb = new StringBuilder();
                blocks.entrySet().stream()
                        .sorted(java.util.Comparator.comparing(e -> e.getKey().getX() * 1000000000L
                                + e.getKey().getY() * 100000L + e.getKey().getZ()))
                        .forEach(e -> sb.append("B ").append(e.getKey().getX()).append(',')
                                .append(e.getKey().getY()).append(',').append(e.getKey().getZ())
                                .append(' ').append(e.getValue().getBlock()).append('\n'));
                System.out.print(sb);
            }
            if (System.getenv("PALE_DUMPDELTA") != null) {                // print cells added THIS draw for the decorator-relevant names
                String[] names = { "minecraft:pale_moss_block", "minecraft:pale_hanging_moss",
                        "minecraft:moss_carpet", "minecraft:pale_oak_log",
                        "minecraft:pale_oak_leaves" };
                for (String n : names) {
                    Block bb = net.minecraft.core.registries.BuiltInRegistries.BLOCK
                            .get(Identifier.parse(n)).map(Holder.Reference::value).orElse(Blocks.AIR);
                    int now = countBlock(blocks, bb);
                    System.out.println("  DELTA " + n.substring(10) + "=" + now);
                }
                for (var e : blocks.entrySet()) {
                    String nm = e.getValue().getBlock().toString();
                    if (nm.contains("hanging_moss") || nm.contains("moss_block")) {
                        System.out.println("  CELL " + e.getKey().getX() + "," + e.getKey().getY()
                                + "," + e.getKey().getZ() + " " + nm);
                    }
                }
            }
            // Debug the real would_survive predicate on the first draw only.
            if (d == 0) {
                for (var mod : chosenHolder.value().placement()) {
                    System.out.println("  inner placement modifier: " + mod);
                }
                Block saplingBlock = net.minecraft.core.registries.BuiltInRegistries.BLOCK
                        .get(Identifier.parse("minecraft:pale_oak_sapling"))
                        .map(Holder.Reference::value).orElse(Blocks.OAK_SAPLING);
                boolean canSurvive = saplingBlock.defaultBlockState()
                        .canSurvive(level, new BlockPos(x, y, z));
                System.out.println("  canSurvive=" + canSurvive
                        + " below=" + level.getBlockState(new BlockPos(x, y - 1, z)));
                try {
                    var pf = chosenHolder.value().placement().get(0);
                    var f = pf.getClass().getDeclaredField("predicate");
                    f.setAccessible(true);
                    Object pred = f.get(pf);
                    System.out.println("  predicate class=" + pred.getClass());
                    var m = pred.getClass().getDeclaredMethod("test",
                            net.minecraft.world.level.WorldGenLevel.class,
                            net.minecraft.core.BlockPos.class);
                    m.setAccessible(true);
                    boolean r = (Boolean) m.invoke(pred, level, new BlockPos(x, y, z));
                    System.out.println("  predicate.test=" + r);
                } catch (Exception e) {
                    System.out.println("  pred debug err: " + e);
                }
            }
            placedAny |= ok;
        }
        System.out.println("placedAny=" + placedAny + " totalNextBits=" + (nextBits - before));

        // Final pale oak trees in the simulated world (trunk bases 2x2).
        int logs = 0;
        Map<String, Integer> treeBases = new HashMap<>();
        Map<Integer, Integer> perTreeLog = new HashMap<>();
        Map<Integer, Integer> perTreeLeaf = new HashMap<>();
        for (Map.Entry<BlockPos, BlockState> e : blocks.entrySet()) {
            BlockPos p = e.getKey();
            if (e.getValue().is(Blocks.PALE_OAK_LOG)) {
                logs++;
                BlockPos nw = new BlockPos(p.getX() - 1, p.getY(), p.getZ() - 1);
                boolean isBase = blocks.getOrDefault(nw, Blocks.AIR.defaultBlockState()).is(Blocks.PALE_OAK_LOG)
                        && blocks.getOrDefault(nw.east(), Blocks.AIR.defaultBlockState()).is(Blocks.PALE_OAK_LOG)
                        && blocks.getOrDefault(nw.south(), Blocks.AIR.defaultBlockState()).is(Blocks.PALE_OAK_LOG)
                        && !blocks.getOrDefault(nw.below(), Blocks.AIR.defaultBlockState()).is(Blocks.PALE_OAK_LOG);
                if (isBase) {
                    String k = nw.getX() + "," + nw.getY() + "," + nw.getZ();
                    treeBases.put(k, treeBases.getOrDefault(k, 0) + 1);
                }
            }
        }
        System.out.println("simulated pale_oak_log=" + logs + " trunkBases=" + treeBases.keySet());
        System.out.println("PER-TREE: " + perTreeLog);
    }

    static int countBlock(Map<BlockPos, BlockState> blocks, Block b) {
        int n = 0;
        for (BlockState st : blocks.values()) {
            if (st.is(b)) n++;
        }
        return n;
    }

    static void bindSupportsVegetationTags() throws Exception {
        var bind = Holder.Reference.class.getDeclaredMethod("bindTags", java.util.Collection.class);
        bind.setAccessible(true);
        var tags = java.util.List.of(net.minecraft.tags.BlockTags.SUPPORTS_VEGETATION);
        for (var b : new Block[] {
                Blocks.DIRT, Blocks.COARSE_DIRT, Blocks.ROOTED_DIRT,
                Blocks.GRASS_BLOCK, Blocks.PODZOL, Blocks.MYCELIUM,
                Blocks.MUD, Blocks.MUDDY_MANGROVE_ROOTS,
                Blocks.MOSS_BLOCK, Blocks.PALE_MOSS_BLOCK, Blocks.FARMLAND }) {
            var holder = net.minecraft.core.registries.BuiltInRegistries.BLOCK.wrapAsHolder(b);
            if (holder instanceof Holder.Reference<?> ref) {
                bind.invoke(ref, tags);
            }
        }
    }

    static boolean wouldSurviveCheck(BlockState below) {
        return below.is(net.minecraft.tags.BlockTags.SUPPORTS_VEGETATION);
    }

    static String belowName(WorldGenLevel level, int x, int y, int z) {
        BlockState s = level.getBlockState(new BlockPos(x, y, z).immutable());
        return s.getBlock().toString();
    }

    static net.minecraft.server.level.ServerLevel serverLevelStub = null;

    static net.minecraft.server.level.ServerLevel makeServerLevel(ChunkGenerator generator)
            throws Exception {
        sun.misc.Unsafe u = unsafe();
        Class<?> wgc = Class.forName("net.minecraft.world.level.chunk.status.WorldGenContext");
        java.lang.reflect.Constructor<?> ctor = wgc.getDeclaredConstructors()[0];
        ctor.setAccessible(true);
        Object wgcInst = ctor.newInstance(null, generator, null, null, null, null);
        Object chunkMap = u.allocateInstance(Class.forName("net.minecraft.server.level.ChunkMap"));
        setField(chunkMap.getClass(), chunkMap, "worldGenContext", wgcInst);
        Object scc = u.allocateInstance(Class.forName("net.minecraft.server.level.ServerChunkCache"));
        setField(scc.getClass(), scc, "chunkMap", chunkMap);
        net.minecraft.server.level.ServerLevel sl = (net.minecraft.server.level.ServerLevel)
                u.allocateInstance(net.minecraft.server.level.ServerLevel.class);
        setField(net.minecraft.server.level.ServerLevel.class, sl, "chunkSource", scc);
        return sl;
    }

    static sun.misc.Unsafe unsafe() throws Exception {
        var f = sun.misc.Unsafe.class.getDeclaredField("theUnsafe");
        f.setAccessible(true);
        return (sun.misc.Unsafe) f.get(null);
    }

    static void setField(Class<?> cls, Object target, String name, Object value) throws Exception {
        java.lang.reflect.Field f = null;
        Class<?> c = cls;
        while (c != null && f == null) {
            try {
                f = c.getDeclaredField(name);
            } catch (NoSuchFieldException e) {
                c = c.getSuperclass();
            }
        }
        if (f == null) throw new NoSuchFieldException(name);
        long offset = unsafe().objectFieldOffset(f);
        unsafe().putObject(target, offset, value);
    }

    static ChunkAccess dummyChunk() {
        try {
            var f = sun.misc.Unsafe.class.getDeclaredField("theUnsafe");
            f.setAccessible(true);
            sun.misc.Unsafe u = (sun.misc.Unsafe) f.get(null);
            return (ChunkAccess) u.allocateInstance(DummyChunk.class);
        } catch (Exception e) {
            throw new RuntimeException(e);
        }
    }

    static class DummyChunk extends ChunkAccess {
        DummyChunk() { super(null, null, null, null, 0L, null, null); }
        public BlockState getBlockState(BlockPos pos) { return Blocks.AIR.defaultBlockState(); }
        public FluidState getFluidState(BlockPos pos) { return Fluids.EMPTY.defaultFluidState(); }
        public net.minecraft.world.level.block.entity.BlockEntity getBlockEntity(BlockPos pos) { return null; }
        public BlockState setBlockState(BlockPos pos, BlockState state, int flags) { return state; }
        public void setBlockEntity(net.minecraft.world.level.block.entity.BlockEntity be) {}
        public void addEntity(net.minecraft.world.entity.Entity e) {}
        public ChunkStatus getPersistedStatus() { return null; }
        public void removeBlockEntity(BlockPos pos) {}
        public net.minecraft.nbt.CompoundTag getBlockEntityNbtForSaving(
                BlockPos pos, net.minecraft.core.HolderLookup.Provider p) { return null; }
        public net.minecraft.world.ticks.TickContainerAccess<Block> getBlockTicks() { return null; }
        public net.minecraft.world.ticks.TickContainerAccess<net.minecraft.world.level.material.Fluid> getFluidTicks() { return null; }
        public ChunkAccess.PackedTicks getTicksForSerialization(long gameTime) { return null; }
    }

    static int height(Map<BlockPos, BlockState> blocks, Heightmap.Types type, int x, int z) {
        for (int y = 319; y >= -64; y--) {
            BlockState s = blocks.getOrDefault(new BlockPos(x, y, z).immutable(),
                    Blocks.AIR.defaultBlockState());
            boolean opaque = switch (type) {
                case WORLD_SURFACE_WG, WORLD_SURFACE -> !s.isAir();
                case OCEAN_FLOOR_WG, OCEAN_FLOOR -> s.blocksMotion();
                case MOTION_BLOCKING -> s.blocksMotion() || !s.getFluidState().isEmpty();
                case MOTION_BLOCKING_NO_LEAVES ->
                        (s.blocksMotion() || !s.getFluidState().isEmpty())
                                && !(s.getBlock() instanceof LeavesBlock);
            };
            if (opaque) return y + 1;
        }
        return -64;
    }

    static ChunkGenerator dummyGenerator = null;

    static net.minecraft.world.level.chunk.ChunkSource dummyChunkSource() {
        return (net.minecraft.world.level.chunk.ChunkSource) Proxy.newProxyInstance(
                net.minecraft.world.level.chunk.ChunkSource.class.getClassLoader(),
                new Class<?>[] {net.minecraft.world.level.chunk.ChunkSource.class},
                (p2, m2, a2) -> {
                    if (m2.getName().equals("getGenerator")) return dummyGenerator;
                    Class<?> r2 = m2.getReturnType();
                    if (r2 == boolean.class) return false;
                    if (r2 == int.class) return 0;
                    if (r2 == long.class) return 0L;
                    if (r2 == void.class) return null;
                    System.err.println("unimplemented ChunkSource." + m2.getName());
                    return null;
                });
    }

    static ChunkGenerator fakeGenerator(RegistryAccess registry, Holder<Biome> paleGarden) {
        return new DummyChunkGenerator();
    }

    static class DummyChunkGenerator extends ChunkGenerator {
        DummyChunkGenerator() {
            super(null, b -> b.value().getGenerationSettings());
        }
        protected com.mojang.serialization.MapCodec<? extends ChunkGenerator> codec() { return null; }
        public void applyCarvers(net.minecraft.server.level.WorldGenRegion r, long l,
                net.minecraft.world.level.levelgen.RandomState s,
                net.minecraft.world.level.biome.BiomeManager m,
                net.minecraft.world.level.StructureManager st,
                ChunkAccess c) {}
        public void buildSurface(net.minecraft.server.level.WorldGenRegion r,
                net.minecraft.world.level.StructureManager st,
                net.minecraft.world.level.levelgen.RandomState s, ChunkAccess c) {}
        public void spawnOriginalMobs(net.minecraft.server.level.WorldGenRegion r) {}
        public int getGenDepth() { return 384; }
        public java.util.concurrent.CompletableFuture<ChunkAccess> fillFromNoise(
                net.minecraft.world.level.levelgen.blending.Blender b,
                net.minecraft.world.level.levelgen.RandomState s,
                net.minecraft.world.level.StructureManager st, ChunkAccess c) {
            return null;
        }
        public int getSeaLevel() { return 63; }
        public int getMinY() { return -64; }
        public int getBaseHeight(int x, int z, net.minecraft.world.level.levelgen.Heightmap.Types t,
                net.minecraft.world.level.LevelHeightAccessor a,
                net.minecraft.world.level.levelgen.RandomState s) { return 0; }
        public net.minecraft.world.level.NoiseColumn getBaseColumn(int x, int z,
                net.minecraft.world.level.LevelHeightAccessor a,
                net.minecraft.world.level.levelgen.RandomState s) { return null; }
        public void addDebugScreenInfo(java.util.List<String> l,
                net.minecraft.world.level.levelgen.RandomState s, BlockPos p) {}
    }

    static WorldGenLevel fakeLevel(Map<BlockPos, BlockState> blocks, ChunkAccess chunk,
            RegistryAccess registry, Holder<Biome> paleGarden) {
        return (WorldGenLevel) Proxy.newProxyInstance(
                WorldGenLevel.class.getClassLoader(),
                new Class<?>[] {WorldGenLevel.class},
                (proxy, method, margs) -> {
                    String n = method.getName();
                    Class<?> ret = method.getReturnType();
                    switch (n) {
                        case "getBlockState" -> {
                            BlockPos p = (BlockPos) margs[0];
                            return blocks.getOrDefault(p.immutable(), Blocks.AIR.defaultBlockState());
                        }
                        case "setBlock" -> {
                            BlockPos p = ((BlockPos) margs[0]).immutable();
                            BlockState s = (BlockState) margs[1];
                            blocks.put(p, s);
                            return true;
                        }
                        case "getFluidState" -> {
                            BlockPos p = (BlockPos) margs[0];
                            BlockState s = blocks.getOrDefault(p.immutable(), Blocks.AIR.defaultBlockState());
                            FluidState f = s.getFluidState();
                            return f != null ? f : Fluids.EMPTY.defaultFluidState();
                        }
                        case "isFluidAtPosition" -> {
                            BlockPos p = (BlockPos) margs[0];
                            @SuppressWarnings("unchecked")
                            java.util.function.Predicate<FluidState> pred = (java.util.function.Predicate<FluidState>) margs[1];
                            BlockState s = blocks.getOrDefault(p.immutable(), Blocks.AIR.defaultBlockState());
                            return pred.test(s.getFluidState());
                        }
                        case "isStateAtPosition" -> {
                            BlockPos p = (BlockPos) margs[0];
                            @SuppressWarnings("unchecked")
                            java.util.function.Predicate<BlockState> pred = (java.util.function.Predicate<BlockState>) margs[1];
                            BlockState s = blocks.getOrDefault(p.immutable(), Blocks.AIR.defaultBlockState());
                            return pred.test(s);
                        }
                        case "getHeight" -> {
                            if (margs != null && margs.length == 3) {
                                Heightmap.Types t = (Heightmap.Types) margs[0];
                                int x = (Integer) margs[1];
                                int z = (Integer) margs[2];
                                return height(blocks, t, x, z);
                            }
                            return 384; // LevelHeightAccessor.getHeight()
                        }
                        case "getMinY" -> { return -64; }
                        case "getMaxY" -> { return 319; }
                        case "getSeaLevel" -> { return 63; }
                        case "getRandom" -> { return net.minecraft.util.RandomSource.create(0); }
                        case "isInsideBuildHeight" -> { return true; }
                        case "ensureCanWrite" -> {
                            BlockPos p = (BlockPos) margs[0];
                            return p.getX() >= -16 && p.getX() < 32 && p.getZ() >= -16 && p.getZ() < 32;
                        }
                        case "isClientSide" -> { return false; }
                        case "isEmptyBlock" -> {
                            BlockPos p = (BlockPos) margs[0];
                            return blocks.getOrDefault(p.immutable(), Blocks.AIR.defaultBlockState()).isAir();
                        }
                        case "getChunk" -> { return chunk; }
                        case "hasChunk" -> { return true; }
                        case "getBiome" -> { return paleGarden; }
                        case "registryAccess" -> { return registry; }
                        case "getLevel" -> { return serverLevelStub; }
                        case "getChunkSource" -> { return dummyChunkSource(); }
                        case "getBlockTicks", "getFluidTicks" -> { return null; }
                        case "playSound", "levelEvent", "addParticle", "destroyBlockProgress" -> { return null; }
                        default -> {
                            if (ret == boolean.class) return false;
                            if (ret == int.class) return 0;
                            if (ret == long.class) return 0L;
                            if (ret == float.class) return 0f;
                            if (ret == double.class) return 0d;
                            if (ret == void.class) return null;
                            System.err.println("unimplemented WorldGenLevel." + n);
                            return null;
                        }
                    }
                });
    }

    static RegistryAccess registryAccessFrom(net.minecraft.core.HolderLookup.Provider lookup) {
        return (RegistryAccess) Proxy.newProxyInstance(
                RegistryAccess.class.getClassLoader(),
                new Class<?>[] {RegistryAccess.class},
                (proxy, method, margs) -> {
                    String n = method.getName();
                    Class<?> ret = method.getReturnType();
                    switch (n) {
                        case "lookup" -> {
                            ResourceKey<?> key = (ResourceKey<?>) margs[0];
                            net.minecraft.core.HolderLookup.RegistryLookup<?> rl =
                                    (net.minecraft.core.HolderLookup.RegistryLookup<?>) (Object)
                                            lookup.lookup((ResourceKey) key).get();
                            return Optional.of(proxyRegistry(rl));
                        }
                        case "lookupOrThrow" -> {
                            ResourceKey<?> key = (ResourceKey<?>) margs[0];
                            return proxyRegistry((net.minecraft.core.HolderLookup.RegistryLookup<?>) (Object)
                                    lookup.lookupOrThrow((ResourceKey) key));
                        }
                        case "compositeAccess" -> { return proxy; }
                    }
                    if (ret == boolean.class) return false;
                    if (ret == int.class) return 0;
                    if (ret == long.class) return 0L;
                    if (ret == float.class) return 0f;
                    if (ret == double.class) return 0d;
                    if (ret == void.class) return null;
                    if (ret == Optional.class) return Optional.empty();
                    System.err.println("unimplemented RegistryAccess." + n);
                    return null;
                });
    }

    static net.minecraft.core.Registry<?> proxyRegistry(net.minecraft.core.HolderLookup.RegistryLookup<?> rl) {
        return (net.minecraft.core.Registry<?>) Proxy.newProxyInstance(
                net.minecraft.core.Registry.class.getClassLoader(),
                new Class<?>[] {net.minecraft.core.Registry.class},
                (proxy, method, margs) -> {
                    String n = method.getName();
                    Class<?> ret = method.getReturnType();
                    switch (n) {
                        case "get" -> {
                            ResourceKey<?> key = (ResourceKey<?>) margs[0];
                            Object r = ((net.minecraft.core.HolderLookup.RegistryLookup) rl)
                                    .get((ResourceKey) key);
                            // Registry.get returns Optional<Holder.Reference<T>>.
                            return r;
                        }
                        case "getValue" -> {
                            Identifier id = (Identifier) margs[0];
                            java.util.Optional<Holder.Reference<?>> ref =
                                    (java.util.Optional<Holder.Reference<?>>) (Object)
                                            ((net.minecraft.core.HolderLookup.RegistryLookup) rl)
                                                    .get(ResourceKey.create(
                                                            net.minecraft.core.registries.Registries.CONFIGURED_FEATURE, id));
                            return ref.map(Holder::value).orElse(null);
                        }
                        case "key" -> {
                            Object v = margs[0];
                            return ((net.minecraft.core.HolderLookup.RegistryLookup) rl)
                                    .listElementIds()
                                    .filter(k -> {
                                        java.util.Optional<Holder.Reference<?>> ref =
                                                (java.util.Optional<Holder.Reference<?>>) (Object)
                                                        ((net.minecraft.core.HolderLookup.RegistryLookup) rl)
                                                                .get((ResourceKey) k);
                                        return ref.map(Holder::value).orElse(null) == v;
                                    })
                                    .findFirst().orElse(null);
                        }
                        case "keyOrThrow" -> {
                            Object v = margs[0];
                            return ((net.minecraft.core.HolderLookup.RegistryLookup) rl)
                                    .listElementIds()
                                    .filter(k -> {
                                        java.util.Optional<Holder.Reference<?>> ref =
                                                (java.util.Optional<Holder.Reference<?>>) (Object)
                                                        ((net.minecraft.core.HolderLookup.RegistryLookup) rl)
                                                                .get((ResourceKey) k);
                                        return ref.map(Holder::value).orElse(null) == v;
                                    })
                                    .findFirst().orElseThrow();
                        }
                    }
                    if (ret == boolean.class) return false;
                    if (ret == int.class) return 0;
                    if (ret == long.class) return 0L;
                    if (ret == float.class) return 0f;
                    if (ret == double.class) return 0d;
                    if (ret == void.class) return null;
                    if (ret == Optional.class) return Optional.empty();
                    if (ret == java.util.Set.class) return java.util.Set.of();
                    if (ret == java.util.stream.Stream.class) return java.util.stream.Stream.empty();
                    if (ret == java.util.Iterator.class) return java.util.Collections.emptyIterator();
                    if (ret == java.util.Collection.class) return java.util.List.of();
                    System.err.println("unimplemented Registry." + n);
                    return null;
                });
    }

    static LevelAccessor fakeLevelAccessor(Map<BlockPos, BlockState> blocks, ChunkAccess chunk,
            RegistryAccess registry) {
        return (LevelAccessor) Proxy.newProxyInstance(
                LevelAccessor.class.getClassLoader(),
                new Class<?>[] {LevelAccessor.class},
                (proxy, method, margs) -> {
                    String n = method.getName();
                    Class<?> ret = method.getReturnType();
                    switch (n) {
                        case "getBlockState" -> {
                            BlockPos p = (BlockPos) margs[0];
                            return blocks.getOrDefault(p.immutable(), Blocks.AIR.defaultBlockState());
                        }
                        case "setBlock" -> {
                            BlockPos p = ((BlockPos) margs[0]).immutable();
                            blocks.put(p, (BlockState) margs[1]);
                            return true;
                        }
                        case "getFluidState" -> {
                            BlockPos p = (BlockPos) margs[0];
                            BlockState s = blocks.getOrDefault(p.immutable(), Blocks.AIR.defaultBlockState());
                            FluidState f = s.getFluidState();
                            return f != null ? f : Fluids.EMPTY.defaultFluidState();
                        }
                        case "getChunk" -> { return chunk; }
                        case "getMinY" -> { return -64; }
                        case "getMaxY" -> { return 319; }
                        case "isClientSide" -> { return false; }
                        case "isEmptyBlock" -> {
                            BlockPos p = (BlockPos) margs[0];
                            return blocks.getOrDefault(p.immutable(), Blocks.AIR.defaultBlockState()).isAir();
                        }
                        case "registryAccess" -> { return registry; }
                        case "getBlockTicks", "getFluidTicks" -> { return null; }
                        default -> {
                            if (ret == boolean.class) return false;
                            if (ret == int.class) return 0;
                            if (ret == long.class) return 0L;
                            if (ret == float.class) return 0f;
                            if (ret == double.class) return 0d;
                            if (ret == void.class) return null;
                            System.err.println("unimplemented LevelAccessor." + n);
                            return null;
                        }
                    }
                });
    }
}