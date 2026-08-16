import java.lang.reflect.Proxy;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import net.minecraft.SharedConstants;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Direction;
import net.minecraft.core.Holder;
import net.minecraft.core.RegistryAccess;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.core.registries.Registries;
import net.minecraft.resources.ResourceKey;
import net.minecraft.resources.Identifier;
import net.minecraft.server.Bootstrap;
import net.minecraft.world.level.LevelAccessor;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.Blocks;
import net.minecraft.world.level.block.SculkSpreader;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.chunk.ChunkAccess;
import net.minecraft.world.level.chunk.status.ChunkStatus;
import net.minecraft.world.level.levelgen.WorldgenRandom;
import net.minecraft.world.level.levelgen.XoroshiroRandomSource;
import net.minecraft.world.level.material.FluidState;
import net.minecraft.world.level.material.Fluids;

/**
 * Full patch-flow differential vs neutron (origin 96,-32): the real
 * SculkSpreader worldgen flow for every gated patch attempt, logging
 * per-attempt draws so the first divergence against neutron is findable.
 *
 * java ProbeSculkFlow [world] [gate] [seed] [ox0] [oz0] [featureIndex]
 */
public class ProbeSculkFlow {
    static int nextBitsCalls = 0;
    static int tickCounter = 0;
    static int patchCounter = 0;
    // FLOW_PATCH: per-cursor draw tracing (CDC lines) + per-tick world dumps
    static int tracePatchIdx = System.getenv("FLOW_PATCH") == null ? -1
            : Integer.parseInt(System.getenv("FLOW_PATCH"));
    static boolean traceDraws = false;
    static String curTag = "-";
    static java.lang.reflect.Field cursorsField;

    static {
        try {
            cursorsField = SculkSpreader.class.getDeclaredField("cursors");
            cursorsField.setAccessible(true);
        } catch (Exception e) {
            throw new RuntimeException(e);
        }
    }

    public static void main(String[] args) throws Exception {
        SharedConstants.tryDetectVersion();
        Bootstrap.bootStrap();
        bindReplaceableTags();

        String worldPath = args.length > 0 ? args[0] : "cave-overlay-3x3.txt";
        String quartsPath = args.length > 1 ? args[1] : "deep-dark-quarts.txt";
        long seed = args.length > 2 ? Long.parseLong(args[2]) : 12345L;
        int ox0 = args.length > 3 ? Integer.parseInt(args[3]) : 96;
        int oz0 = args.length > 4 ? Integer.parseInt(args[4]) : -32;
        int featureIndex = args.length > 5 ? Integer.parseInt(args[5]) : 1;

        Map<BlockPos, BlockState> blocks = new HashMap<>();
        loadWorld(worldPath, blocks);
        System.out.println("loaded " + worldPath + " cells=" + blocks.size());

        java.util.Set<Long> dd = new java.util.HashSet<>();
        for (String line : Files.readAllLines(Path.of(quartsPath))) {
            line = line.trim();
            if (line.isEmpty()) continue;
            String[] q = line.split("\s+");
            long key = (long) Integer.parseInt(q[0]) << 40 | (Integer.parseInt(q[1]) & 0xFFFFFL) << 20
                    | (Integer.parseInt(q[2]) & 0xFFFFFL);
            dd.add(key);
        }
        System.out.println("quarts=" + dd.size());

        ChunkAccess dummyChunk = dummyChunk();
        LevelAccessor level = fakeLevel(blocks, dummyChunk);

        WorldgenRandom rng = new WorldgenRandom(new XoroshiroRandomSource(seed)) {
            @Override public int next(int bits) {
                nextBitsCalls++;
                return super.next(bits);
            }
            @Override public int nextInt(int bound) {
                int v = super.nextInt(bound);
                if (traceDraws) {
                    System.out.println("DRAW " + nextBitsCalls + " " + curTag
                        + " nextInt(" + bound + ")=" + v);
                }
                return v;
            }
            @Override public float nextFloat() {
                float v = super.nextFloat();
                if (traceDraws) {
                    System.out.println("DRAW " + nextBitsCalls + " " + curTag + " nextFloat");
                }
                return v;
            }
        };
        rng.setSeed(seed);
        long a = rng.nextLong() | 1L;
        long b = rng.nextLong() | 1L;
        long dec = (long) ox0 * a + (long) oz0 * b ^ seed;
        rng.setFeatureSeed(dec, featureIndex, 7);

        int ran = 0;
        for (int i = 0; i < 256; i++) {
            tickCounter = 0;
            patchCounter = i;
            int x = ox0 + rng.nextInt(16);
            int z = oz0 + rng.nextInt(16);
            int y = -64 + rng.nextInt(321);
            long key = (long) (x >> 2) << 40 | ((y >> 2) & 0xFFFFFL) << 20 | ((z >> 2) & 0xFFFFFL);
            boolean biomeOk = dd.contains(key);
            System.out.println("ATT i=" + i + " " + x + "," + y + "," + z + " biome=" + (biomeOk ? 1 : 0)
                + " here=" + level.getBlockState(new BlockPos(x, y, z)).getBlock());
            if (!biomeOk) continue;
            BlockPos origin = new BlockPos(x, y, z);
            if (!canSpreadFrom(level, origin)) {
                continue;
            }
            int before = nextBitsCalls;
            SculkSpreader spreader = SculkSpreader.createWorldGenSpreader();
            for (int c = 0; c < 10; c++) {
                spreader.addCursors(origin, 32);
            }
            for (int att = 0; att < 64; att++) {
                tickCounter++;
                boolean trace = tracePatchIdx == i;
                traceDraws = trace;
                if (trace) {
                    updateCursorsTraced(spreader, level, origin, rng, i, att);
                } else {
                    spreader.updateCursors(level, origin, rng, true);
                }
                if (trace) {
                    // Full world dump after this update (neutron TICKDUMP mirror).
                    List<String> cells = worldCells(blocks);
                    Files.write(Path.of("java-tickfull-" + i + "-" + att + ".txt"), cells);
                    for (var c : spreader.getCursors()) {
                        var cp = c.getPos();
                        int upd;
                        try {
                            var uf = SculkSpreader.ChargeCursor.class.getDeclaredField("updateDelay");
                            uf.setAccessible(true);
                            upd = uf.getInt(c);
                        } catch (Exception e) {
                            upd = -1;
                        }
                        System.out.println("CURA i=" + i + " att=" + att + " " + cp.getX() + ","
                                + cp.getY() + "," + cp.getZ() + " ch=" + c.getCharge() + " dec="
                                + c.getDecayDelay() + " upd=" + upd + " faces="
                                + packFaces(c.getFacingData()));
                    }
                }
                if (System.getenv("FLOW_CURSORS") != null && att <= 38) {
                    for (var c : spreader.getCursors()) {
                        var cp = c.getPos();
                        var st = level.getBlockState(cp);
                        System.out.println("CUR i=" + i + " att=" + att + " " + cp.getX() + ","
                                + cp.getY() + "," + cp.getZ() + " ch=" + c.getCharge() + " dec="
                                + c.getDecayDelay() + " faces=" + c.getFacingData() + " blk="
                                + st.getBlock());
                    }
                }
                if (System.getenv("SCULK_DUMP_TICK") != null && (att == 36 || att == 4)) {
                    List<String> cells = new ArrayList<>();
                    for (Map.Entry<BlockPos, BlockState> e : blocks.entrySet()) {
                        BlockState st = e.getValue();
                        if (st.is(Blocks.SCULK)) {
                            cells.add(e.getKey().getX() + "," + e.getKey().getY() + "," + e.getKey().getZ() + " sculk#0");
                        } else if (st.is(Blocks.SCULK_VEIN)) {
                            int m = 0;
                            for (int d = 0; d < 6; d++) {
                                if (net.minecraft.world.level.block.MultifaceBlock.hasFace(
                                        st, Direction.values()[d])) m |= 1 << d;
                            }
                            cells.add(e.getKey().getX() + "," + e.getKey().getY() + "," + e.getKey().getZ()
                                    + " vein#" + m);
                        }
                    }
                    cells.sort(String::compareTo);
                    Files.write(Path.of("java-tick-" + i + "-" + att + ".txt"), cells);
                }
                if (System.getenv("FLOW_TICKS") != null) {
                    int ts = 0, tv = 0;
                    for (Map.Entry<BlockPos, BlockState> e : blocks.entrySet()) {
                        if (e.getValue().is(Blocks.SCULK)) ts++;
                        else if (e.getValue().is(Blocks.SCULK_VEIN)) tv++;
                    }
                    System.out.println("TICK i=" + i + " att=" + att + " cursors="
                            + spreader.getCursors().size() + " sculk=" + ts + " vein=" + tv
                            + " draws=" + nextBitsCalls);
                }
            }
            spreader.clear();
            float roll = rng.nextFloat();
            BlockPos below = origin.below();
            if (roll <= 0.5f && level.getBlockState(below).isCollisionShapeFullBlock(level, below)) {
                level.setBlock(origin, Blocks.SCULK_CATALYST.defaultBlockState(), 3);
            }
            int sc = 0, vn = 0;
            for (Map.Entry<BlockPos, BlockState> e : blocks.entrySet()) {
                if (e.getValue().is(Blocks.SCULK)) sc++;
                else if (e.getValue().is(Blocks.SCULK_VEIN)) vn++;
            }
            System.out.println("RUN i=" + i + " " + x + "," + y + "," + z
                    + " draws=" + (nextBitsCalls - before) + " sculk=" + sc + " vein=" + vn
                    + " roll=" + Float.toString(roll));
            if (System.getenv("SCULK_DUMP_TICK") != null) {
                // handled below via per-tick dumps at tick 37
            }
            if (System.getenv("SCULK_DUMP_PATCH") != null) {
                List<String> cells = new ArrayList<>();
                for (Map.Entry<BlockPos, BlockState> e : blocks.entrySet()) {
                    BlockState st = e.getValue();
                    if (st.is(Blocks.SCULK)) {
                        cells.add(e.getKey().getX() + "," + e.getKey().getY() + "," + e.getKey().getZ() + " sculk#0");
                    } else if (st.is(Blocks.SCULK_VEIN)) {
                        int m = 0;
                        for (int d = 0; d < 6; d++) {
                            if (net.minecraft.world.level.block.MultifaceBlock.hasFace(
                                    st, Direction.values()[d])) m |= 1 << d;
                        }
                        cells.add(e.getKey().getX() + "," + e.getKey().getY() + "," + e.getKey().getZ()
                                + " vein#" + m);
                    }
                }
                cells.sort(String::compareTo);
                Files.write(Path.of("java-patch-" + i + ".txt"), cells);
            }
            ran++;
        }
        System.out.println("ran=" + ran + " totalDraws=" + nextBitsCalls);
    }

    static int packFaces(java.util.Collection<Direction> faces) {
        int m = 0;
        if (faces == null) return -1;
        for (Direction d : faces) m |= 1 << d.ordinal();
        return m;
    }

    static List<String> worldCells(Map<BlockPos, BlockState> blocks) {
        List<String> cells = new ArrayList<>();
        for (Map.Entry<BlockPos, BlockState> e : blocks.entrySet()) {
            BlockState st = e.getValue();
            if (st.is(Blocks.SCULK)) {
                cells.add(e.getKey().getX() + "," + e.getKey().getY() + "," + e.getKey().getZ() + " sculk#0");
            } else if (st.is(Blocks.SCULK_VEIN)) {
                int m = 0;
                for (int d = 0; d < 6; d++) {
                    if (net.minecraft.world.level.block.MultifaceBlock.hasFace(st, Direction.values()[d])) m |= 1 << d;
                }
                cells.add(e.getKey().getX() + "," + e.getKey().getY() + "," + e.getKey().getZ()
                        + " vein#" + m);
            }
        }
        cells.sort(String::compareTo);
        return cells;
    }

    /// Faithful copy of SculkSpreader.updateCursors (worldgen branch: no merge)
    /// with per-cursor draw accounting. levelEvent(3006) particles are skipped.
    @SuppressWarnings("unchecked")
    static void updateCursorsTraced(SculkSpreader spreader, LevelAccessor level, BlockPos origin,
            WorldgenRandom rng, int patch, int att) {
        List<SculkSpreader.ChargeCursor> cursors;
        try {
            cursors = (List<SculkSpreader.ChargeCursor>) cursorsField.get(spreader);
        } catch (Exception e) {
            throw new RuntimeException(e);
        }
        if (cursors.isEmpty()) {
            return;
        }
        ArrayList<SculkSpreader.ChargeCursor> processed = new ArrayList<>();
        int idx = 0;
        var uf = new Object() { java.lang.reflect.Field f; };
        try {
            uf.f = SculkSpreader.ChargeCursor.class.getDeclaredField("updateDelay");
            uf.f.setAccessible(true);
        } catch (Exception e) {
            throw new RuntimeException(e);
        }
        for (SculkSpreader.ChargeCursor cursor : new ArrayList<>(cursors)) {
            BlockPos cp = cursor.getPos();
            if (cp.distChessboard(origin) > 1024) {
                idx++;
                continue;
            }
            int upd;
            try {
                upd = uf.f.getInt(cursor);
            } catch (Exception e) {
                upd = -1;
            }
            curTag = "i=" + patch + " att=" + att + " cur=" + idx;
            int before = nextBitsCalls;
            System.out.println("CDCB " + curTag + " " + cp.getX() + "," + cp.getY() + "," + cp.getZ()
                    + " ch=" + cursor.getCharge() + " dec=" + cursor.getDecayDelay() + " upd=" + upd
                    + " faces=" + packFaces(cursor.getFacingData()));
            cursor.update(level, origin, rng, spreader, true);
            int n = nextBitsCalls - before;
            BlockPos np = cursor.getPos();
            System.out.println("CDCE " + curTag + " n=" + n + " ch=" + cursor.getCharge()
                    + " pos=" + np.getX() + "," + np.getY() + "," + np.getZ()
                    + " faces=" + packFaces(cursor.getFacingData()));
            curTag = "-";
            if (cursor.getCharge() > 0) {
                processed.add(cursor);
            }
            idx++;
        }
        try {
            cursorsField.set(spreader, processed);
        } catch (Exception e) {
            throw new RuntimeException(e);
        }
    }

    static boolean canSpreadFrom(LevelAccessor level, BlockPos origin) {
        BlockState start = level.getBlockState(origin);
        if (start.getBlock() instanceof net.minecraft.world.level.block.SculkBehaviour) {
            return true;
        }
        if (start.isAir()) {
            return Direction.stream().map(origin::relative)
                    .anyMatch(pos -> level.getBlockState(pos).isCollisionShapeFullBlock(level, pos));
        }
        if (start.is(Blocks.WATER) && start.getFluidState().isSource()) {
            return Direction.stream().map(origin::relative)
                    .anyMatch(pos -> level.getBlockState(pos).isCollisionShapeFullBlock(level, pos));
        }
        return false;
    }

    static void loadWorld(String path, Map<BlockPos, BlockState> blocks) throws Exception {
        for (String line : Files.readAllLines(Path.of(path))) {
            line = line.trim();
            if (line.isEmpty() || line.startsWith("#")) continue;
            String[] p = line.split("\\s+");
            if (p[0].equals("origin")) continue;
            int x = Integer.parseInt(p[0]);
            int y = Integer.parseInt(p[1]);
            int z = Integer.parseInt(p[2]);
            BlockState st = blockByName(p[3]);
            if (st.isAir()) continue;
            blocks.put(new BlockPos(x, y, z), st);
        }
    }

    static BlockState blockByName(String n) {
        return BuiltInRegistries.BLOCK.get(
            ResourceKey.create(Registries.BLOCK, Identifier.withDefaultNamespace(n))).get()
            .value().defaultBlockState();
    }

    static void bindReplaceableTags() throws Exception {
        var bind = Holder.Reference.class.getDeclaredMethod("bindTags", java.util.Collection.class);
        bind.setAccessible(true);
        var tags = java.util.List.of(
                net.minecraft.tags.BlockTags.SCULK_REPLACEABLE,
                net.minecraft.tags.BlockTags.SCULK_REPLACEABLE_WORLD_GEN);
        for (var b : new Block[] {
                Blocks.DEEPSLATE, Blocks.STONE, Blocks.TUFF, Blocks.GRANITE,
                Blocks.DIORITE, Blocks.ANDESITE, Blocks.DIRT, Blocks.GRAVEL, Blocks.CALCITE }) {
            var holder = BuiltInRegistries.BLOCK.wrapAsHolder(b);
            if (holder instanceof Holder.Reference<?> ref) {
                bind.invoke(ref, tags);
            }
        }
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

    static LevelAccessor fakeLevel(Map<BlockPos, BlockState> blocks, ChunkAccess chunk) {
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
                            BlockState s = (BlockState) margs[1];
                            String tc = System.getenv("TRACE_COORD");
                            if (tc != null) {
                                boolean hit = false;
                                for (String cc : tc.split(";")) {
                                    String[] q = cc.split(",");
                                    if (p.getX() == Integer.parseInt(q[0])
                                            && p.getY() == Integer.parseInt(q[1])
                                            && p.getZ() == Integer.parseInt(q[2])) {
                                        hit = true;
                                        break;
                                    }
                                }
                                if (hit) {
                                    StringBuilder m = new StringBuilder();
                                    for (int d = 0; d < 6; d++) {
                                        if (net.minecraft.world.level.block.MultifaceBlock.hasFace(
                                                s, Direction.values()[d])) m.append(' ').append(d);
                                    }
                                    System.out.println("TRACE setBlock p" + patchCounter + " t" + tickCounter
                                            + " " + p.getX() + "," + p.getY()
                                            + "," + p.getZ() + " " + s.getBlock() + " faces" + m);
                                    new Exception("STACK").printStackTrace(System.out);
                                }
                            }
                            blocks.put(p, s);
                            return true;
                        }
                        case "getFluidState" -> {
                            BlockPos p = (BlockPos) margs[0];
                            BlockState s = blocks.getOrDefault(p.immutable(), Blocks.AIR.defaultBlockState());
                            FluidState f = s.getFluidState();
                            return f != null ? f : Fluids.EMPTY.defaultFluidState();
                        }
                        case "playSound" -> { return null; }
                        case "getChunk" -> { return chunk; }
                        case "getMinY" -> { return -64; }
                        case "getMaxY" -> { return 319; }
                        case "isClientSide" -> { return false; }
                        case "isEmptyBlock" -> {
                            BlockPos p = (BlockPos) margs[0];
                            return blocks.getOrDefault(p.immutable(), Blocks.AIR.defaultBlockState()).isAir();
                        }
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
