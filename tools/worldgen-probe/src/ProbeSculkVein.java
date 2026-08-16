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
import net.minecraft.core.HolderSet;
import net.minecraft.core.RegistryAccess;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.core.registries.Registries;
import net.minecraft.resources.ResourceKey;
import net.minecraft.resources.Identifier;
import net.minecraft.server.Bootstrap;
import net.minecraft.world.level.ChunkPos;
import net.minecraft.world.level.LevelAccessor;
import net.minecraft.world.level.WorldGenLevel;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.Blocks;
import net.minecraft.world.level.block.MultifaceBlock;
import net.minecraft.world.level.block.MultifaceSpreadeableBlock;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.chunk.ChunkAccess;
import net.minecraft.world.level.chunk.UpgradeData;
import net.minecraft.world.level.chunk.status.ChunkStatus;
import net.minecraft.world.level.levelgen.WorldgenRandom;
import net.minecraft.world.level.levelgen.XoroshiroRandomSource;
import net.minecraft.world.level.levelgen.feature.MultifaceGrowthFeature;
import net.minecraft.world.level.levelgen.feature.configurations.MultifaceGrowthConfiguration;
import net.minecraft.world.level.material.FluidState;
import net.minecraft.world.level.material.Fluids;

/**
 * Vein-feature differential vs neutron sculk_veintrace: same world dump, same
 * gate decisions (vein-gate file), real MultifaceGrowthFeature code paths.
 *
 * java ProbeSculkVein [dump] [gateFile] [seed] [ox0] [oz0] [featureIndex]
 */
public class ProbeSculkVein {
    public static void main(String[] args) throws Exception {
        SharedConstants.tryDetectVersion();
        Bootstrap.bootStrap();
        bindReplaceableTags();

        String dumpPath = args.length > 0 ? args[0] : "cave-98-43-23.txt";
        String gatePath = args.length > 1 ? args[1] : "vein-gate-96--32.txt";
        long seed = args.length > 2 ? Long.parseLong(args[2]) : 12345L;
        int ox0 = args.length > 3 ? Integer.parseInt(args[3]) : 96;
        int oz0 = args.length > 4 ? Integer.parseInt(args[4]) : -32;
        int featureIndex = args.length > 5 ? Integer.parseInt(args[5]) : 0;

        Map<BlockPos, BlockState> blocks = new HashMap<>();
        loadCave(dumpPath, blocks);
        System.out.println("loaded " + dumpPath + " cells=" + blocks.size());

        List<int[]> gate = new ArrayList<>();
        for (String line : Files.readAllLines(Path.of(gatePath))) {
            line = line.trim();
            if (line.isEmpty()) continue;
            String[] p = line.split("\\s+");
            gate.add(new int[] {
                Integer.parseInt(p[0]), Integer.parseInt(p[1]), Integer.parseInt(p[2]),
                Integer.parseInt(p[3])});
        }
        System.out.println("gate entries=" + gate.size());

        ChunkAccess dummyChunk = dummyChunk();
        WorldGenLevel level = fakeLevel(blocks, dummyChunk);

        WorldgenRandom rng = new WorldgenRandom(new XoroshiroRandomSource(seed));
        rng.setSeed(seed);
        long a = rng.nextLong() | 1L;
        long b = rng.nextLong() | 1L;
        long dec = (long) ox0 * a + (long) oz0 * b ^ seed;
        rng.setFeatureSeed(dec, featureIndex, 7);

        MultifaceGrowthConfiguration cfg = new MultifaceGrowthConfiguration(
            Blocks.SCULK_VEIN, 20, true, true, true, 1.0f, canBePlacedOn());
        MultifaceSpreadeableBlock placer = (MultifaceSpreadeableBlock) Blocks.SCULK_VEIN;

        int count = 204 + rng.nextInt(47);
        System.out.println("count=" + count);
        int placed = 0, solid = 0;
        for (int[] g : gate) {
            rng.nextInt(16);
            rng.nextInt(16);
            rng.nextInt(321);
            if (g[3] == 0) continue;
            BlockPos origin = new BlockPos(g[0], g[1], g[2]);
            if (!isAirOrWater(level.getBlockState(origin))) {
                System.out.println("SOLID " + g[0] + "," + g[1] + "," + g[2]);
                solid++;
                continue;
            }
            List<Direction> searchDirs = cfg.getShuffledDirections(rng);
            if (MultifaceGrowthFeature.placeGrowthIfPossible(
                    placer, level, origin, level.getBlockState(origin), cfg, rng, searchDirs)) {
                System.out.println("PLACED " + g[0] + "," + g[1] + "," + g[2] + "#" + maskAt(level, origin));
                placed++;
                continue;
            }
            BlockPos.MutableBlockPos pos = origin.mutable();
            boolean done = false;
            outer:
            for (Direction sd : searchDirs) {
                List<Direction> placementDirs = cfg.getShuffledDirectionsExcept(rng, sd.getOpposite());
                for (int i = 0; i < cfg.searchRange; ++i) {
                    pos.setWithOffset(origin, sd);
                    BlockState state = level.getBlockState(pos);
                    if (!isAirOrWater(state) && !state.is(cfg.placeBlock)) continue outer;
                    if (MultifaceGrowthFeature.placeGrowthIfPossible(
                            placer, level, pos, state, cfg, rng, placementDirs)) {
                        System.out.println("PLACED " + pos.getX() + "," + pos.getY() + "," + pos.getZ()
                                + "#" + maskAt(level, pos));
                        placed++;
                        done = true;
                        break outer;
                    }
                }
            }
            if (!done) System.out.println("FAILED " + g[0] + "," + g[1] + "," + g[2]);
        }
        System.out.println("placed=" + placed + " solid=" + solid);

        List<String> cells = new ArrayList<>();
        for (Map.Entry<BlockPos, BlockState> e : blocks.entrySet()) {
            if (e.getValue().is(Blocks.SCULK_VEIN)) {
                cells.add(e.getKey().getX() + "," + e.getKey().getY() + "," + e.getKey().getZ()
                        + "#" + maskState(e.getValue()));
            }
        }
        cells.sort(String::compareTo);
        System.out.println("final vein cells=" + cells.size());
        for (String c : cells) System.out.println("VEIN " + c);
    }

    static int maskAt(LevelAccessor level, BlockPos p) {
        return maskState(level.getBlockState(p));
    }

    static int maskState(BlockState st) {
        int m = 0;
        for (int i = 0; i < 6; i++) {
            if (MultifaceBlock.hasFace(st, Direction.values()[i])) m |= 1 << i;
        }
        return m;
    }

    static HolderSet<Block> canBePlacedOn() {
        String[] names = {
            "stone", "andesite", "diorite", "granite", "dripstone_block", "calcite", "tuff", "deepslate"};
        List<Holder<Block>> holders = new ArrayList<>();
        for (String n : names) {
            holders.add(BuiltInRegistries.BLOCK.get(
                ResourceKey.create(Registries.BLOCK, Identifier.withDefaultNamespace(n))).get());
        }
        return HolderSet.direct(holders);
    }

    static boolean isAirOrWater(BlockState state) {
        return state.isAir() || state.is(Blocks.WATER);
    }

    static void loadCave(String path, Map<BlockPos, BlockState> blocks) throws Exception {
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

    static void bindReplaceableTags() {
        try {
            var bind =
                    net.minecraft.core.Holder.Reference.class.getDeclaredMethod(
                            "bindTags", java.util.Collection.class);
            bind.setAccessible(true);
            var tags =
                    java.util.List.of(
                            net.minecraft.tags.BlockTags.SCULK_REPLACEABLE,
                            net.minecraft.tags.BlockTags.SCULK_REPLACEABLE_WORLD_GEN);
            for (var b :
                    new net.minecraft.world.level.block.Block[] {
                        Blocks.DEEPSLATE,
                        Blocks.STONE,
                        Blocks.TUFF,
                        Blocks.GRANITE,
                        Blocks.DIORITE,
                        Blocks.ANDESITE,
                        Blocks.DIRT,
                        Blocks.GRAVEL,
                        Blocks.CALCITE
                    }) {
                var holder = BuiltInRegistries.BLOCK.wrapAsHolder(b);
                if (holder instanceof net.minecraft.core.Holder.Reference<?> ref) {
                    bind.invoke(ref, tags);
                }
            }
        } catch (Exception e) {
            throw new RuntimeException(e);
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
        DummyChunk() {
            super(null, null, null, null, 0L, null, null);
        }

        public BlockState getBlockState(BlockPos pos) { return Blocks.AIR.defaultBlockState(); }
        public FluidState getFluidState(BlockPos pos) { return Fluids.EMPTY.defaultFluidState(); }
        public net.minecraft.world.level.block.entity.BlockEntity getBlockEntity(BlockPos pos) { return null; }
        public BlockState setBlockState(BlockPos pos, BlockState state, int flags) { return state; }
        public void setBlockEntity(net.minecraft.world.level.block.entity.BlockEntity be) {}
        public void addEntity(net.minecraft.world.entity.Entity e) {}
        public ChunkStatus getPersistedStatus() { return null; }
        public void removeBlockEntity(BlockPos pos) {}
        public net.minecraft.nbt.CompoundTag getBlockEntityNbtForSaving(BlockPos pos, net.minecraft.core.HolderLookup.Provider p) { return null; }
        public net.minecraft.world.ticks.TickContainerAccess<Block> getBlockTicks() { return null; }
        public net.minecraft.world.ticks.TickContainerAccess<net.minecraft.world.level.material.Fluid> getFluidTicks() { return null; }
        public ChunkAccess.PackedTicks getTicksForSerialization(long gameTime) { return null; }
    }

    static WorldGenLevel fakeLevel(Map<BlockPos, BlockState> blocks, ChunkAccess chunk) {
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
