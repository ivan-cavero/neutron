import java.lang.reflect.Proxy;
import java.util.HashMap;
import java.util.Map;
import net.minecraft.SharedConstants;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Direction;
import net.minecraft.server.Bootstrap;
import net.minecraft.core.RegistryAccess;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.world.level.ChunkPos;
import net.minecraft.world.level.LevelAccessor;
import net.minecraft.world.level.block.Blocks;
import net.minecraft.world.level.block.SculkSpreader;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.chunk.ChunkAccess;
import net.minecraft.world.level.chunk.PalettedContainerFactory;
import net.minecraft.world.level.chunk.ProtoChunk;
import net.minecraft.world.level.chunk.UpgradeData;
import net.minecraft.world.level.levelgen.WorldgenRandom;
import net.minecraft.world.level.levelgen.XoroshiroRandomSource;
import net.minecraft.world.level.material.FluidState;
import net.minecraft.world.level.material.Fluids;

/**
 * Flat-floor SculkPatch vs Neutron run_patch: count RNG + catalyst nextFloat.
 *
 * World: y=9 deepslate, y=10 air, origin (8,10,8). charge 10×32, 64 attempts.
 */
public class ProbeSculkPatch {
    static int nextIntCalls = 0;
    static int nextFloatCalls = 0;
    static int nextBitsCalls = 0;

    public static void main(String[] args) {
        SharedConstants.tryDetectVersion();
        Bootstrap.bootStrap();
        bindReplaceableTags();
        System.out.println(
                "deepslate_in_world_gen_tag="
                        + Blocks.DEEPSLATE
                                .defaultBlockState()
                                .is(net.minecraft.tags.BlockTags.SCULK_REPLACEABLE_WORLD_GEN));

        Map<BlockPos, BlockState> blocks = new HashMap<>();
        BlockPos origin = new BlockPos(8, 10, 8);
        if (args.length > 0) {
            origin = loadCave(args[0], blocks);
            System.out.println("loaded cave " + args[0] + " origin=" + origin + " cells=" + blocks.size());
        } else {
            for (int z = -32; z < 48; z++) {
                for (int x = -32; x < 48; x++) {
                    blocks.put(new BlockPos(x, 9, z), Blocks.DEEPSLATE.defaultBlockState());
                    blocks.put(new BlockPos(x, 10, z), Blocks.AIR.defaultBlockState());
                }
            }
        }

        ChunkAccess dummyChunk = dummyChunk();
        LevelAccessor level = fakeLevel(blocks, dummyChunk);
        WorldgenRandom rng = new WorldgenRandom(new XoroshiroRandomSource(1L)) {
            @Override
            public int next(int bits) {
                nextBitsCalls++;
                return super.next(bits);
            }

            @Override
            public int nextInt(int bound) {
                nextIntCalls++;
                return super.nextInt(bound);
            }

            @Override
            public float nextFloat() {
                nextFloatCalls++;
                return super.nextFloat();
            }
        };

        SculkSpreader spreader = SculkSpreader.createWorldGenSpreader();
        for (int i = 0; i < 10; i++) {
            spreader.addCursors(origin, 32);
        }
        for (int a = 0; a < 64; a++) {
            if (spreader.getCursors().isEmpty()) {
                System.out.println("cursors empty at attempt " + a);
                break;
            }
            spreader.updateCursors(level, origin, rng, true);
            if (a < 8 || a == 15 || a == 31 || a == 63) {
                int sc = 0, vn = 0;
                for (var e : blocks.entrySet()) {
                    var b = e.getValue().getBlock();
                    if (b == Blocks.SCULK) sc++;
                    else if (b == Blocks.SCULK_VEIN) vn++;
                }
                System.out.println(
                        "after "
                                + (a + 1)
                                + " cursors="
                                + spreader.getCursors().size()
                                + " sculk="
                                + sc
                                + " vein="
                                + vn
                                + " nextBits="
                                + nextBitsCalls);
                if (a == 2) {
                    System.out.print("after3 vein:");
                    for (var e : blocks.entrySet()) {
                        if (e.getValue().is(Blocks.SCULK_VEIN)) {
                            var p = e.getKey();
                            var st = e.getValue();
                            int m = 0;
                            for (int i = 0; i < 6; i++) {
                                if (net.minecraft.world.level.block.MultifaceBlock.hasFace(
                                        st, Direction.values()[i])) {
                                    m |= 1 << i;
                                }
                            }
                            System.out.print(" " + p.getX() + "," + p.getY() + "," + p.getZ() + "#" + m);
                        }
                    }
                    System.out.println();
                    System.out.print("after3 sculk:");
                    for (var e : blocks.entrySet()) {
                        if (e.getValue().is(Blocks.SCULK)) {
                            var p = e.getKey();
                            System.out.print(" " + p.getX() + "," + p.getY() + "," + p.getZ());
                        }
                    }
                    System.out.println();
                    System.out.println(
                            "after3 cell 7,10,8="
                                    + blocks.getOrDefault(new BlockPos(7, 10, 8), Blocks.AIR.defaultBlockState())
                                            .getBlock()
                                    + " 8,10,9="
                                    + blocks.getOrDefault(new BlockPos(8, 10, 9), Blocks.AIR.defaultBlockState())
                                            .getBlock());
                    for (var c : spreader.getCursors()) {
                        var p = c.getPos();
                        System.out.println(
                                "  live "
                                        + p.getX()
                                        + ","
                                        + p.getY()
                                        + ","
                                        + p.getZ()
                                        + " ch="
                                        + c.getCharge());
                    }
                }
                if (a < 1) {
                    System.out.print("after1 vein:");
                    for (var e : blocks.entrySet()) {
                        if (e.getValue().is(Blocks.SCULK_VEIN)) {
                            var p = e.getKey();
                            var st = e.getValue();
                            int m = 0;
                            for (int i = 0; i < 6; i++) {
                                if (net.minecraft.world.level.block.MultifaceBlock.hasFace(
                                        st, Direction.values()[i])) {
                                    m |= 1 << i;
                                }
                            }
                            System.out.print(" " + p.getX() + "," + p.getY() + "," + p.getZ() + "#" + m);
                        }
                    }
                    System.out.println();
                    System.out.print("after1 sculk:");
                    for (var e : blocks.entrySet()) {
                        if (e.getValue().is(Blocks.SCULK)) {
                            var p = e.getKey();
                            System.out.print(" " + p.getX() + "," + p.getY() + "," + p.getZ());
                        }
                    }
                    System.out.println();
                }
                if (false) {
                    for (var c : spreader.getCursors()) {
                        var p = c.getPos();
                        var st = blocks.getOrDefault(p, Blocks.AIR.defaultBlockState());
                        System.out.println(
                                "  cur "
                                        + p.getX()
                                        + ","
                                        + p.getY()
                                        + ","
                                        + p.getZ()
                                        + " ch="
                                        + c.getCharge()
                                        + " dec="
                                        + c.getDecayDelay()
                                        + " faces="
                                        + c.getFacingData()
                                        + " blk="
                                        + st.getBlock());
                    }
                    System.out.println("vein faces:");
                    for (var e : blocks.entrySet()) {
                        if (e.getValue().is(Blocks.SCULK_VEIN)) {
                            var p = e.getKey();
                            var st = e.getValue();
                            StringBuilder sb = new StringBuilder();
                            for (Direction d : Direction.values()) {
                                if (net.minecraft.world.level.block.MultifaceBlock.hasFace(st, d)) {
                                    sb.append(' ').append(d);
                                }
                            }
                            System.out.println(
                                    "  vein " + p.getX() + "," + p.getY() + "," + p.getZ() + sb);
                        }
                    }
                }
            }
        }
        spreader.clear();

        float roll = rng.nextFloat();
        BlockState below = blocks.getOrDefault(origin.below(), Blocks.AIR.defaultBlockState());
        boolean full = below.isCollisionShapeFullBlock(level, origin.below());
        System.out.println("catalyst_roll=" + Float.toString(roll));
        System.out.println("catalyst_place=" + (roll <= 0.5f && full) + " below=" + below.getBlock());
        System.out.println("nextInt=" + nextIntCalls + " nextFloat=" + nextFloatCalls + " nextBits=" + nextBitsCalls);

        int sculk = 0, vein = 0, sensor = 0, shrieker = 0, cat = 0;
        for (Map.Entry<BlockPos, BlockState> e : blocks.entrySet()) {
            var b = e.getValue().getBlock();
            if (b == Blocks.SCULK) sculk++;
            else if (b == Blocks.SCULK_VEIN) vein++;
            else if (b == Blocks.SCULK_SENSOR) sensor++;
            else if (b == Blocks.SCULK_SHRIEKER) shrieker++;
            else if (b == Blocks.SCULK_CATALYST) cat++;
        }
        System.out.println("sculk=" + sculk + " vein=" + vein + " sensor=" + sensor + " shrieker=" + shrieker + " cat=" + cat);

        BlockPos at = origin.below();
        System.out.println("at_origin_below=" + blocks.getOrDefault(at, Blocks.AIR.defaultBlockState()).getBlock());
        System.out.println("at_origin=" + blocks.getOrDefault(origin, Blocks.AIR.defaultBlockState()).getBlock());
    }

    /** Concrete ChunkAccess that never runs its ctor; only markPosForPostProcessing is used. */
    static class DummyChunk extends ChunkAccess {
        DummyChunk() {
            super(null, null, null, null, 0L, null, null);
        }

        public BlockState getBlockState(BlockPos pos) {
            return Blocks.AIR.defaultBlockState();
        }

        public FluidState getFluidState(BlockPos pos) {
            return Fluids.EMPTY.defaultFluidState();
        }

        public net.minecraft.world.level.block.entity.BlockEntity getBlockEntity(BlockPos pos) {
            return null;
        }

        public BlockState setBlockState(BlockPos pos, BlockState state, int flags) {
            return state;
        }

        public void setBlockEntity(net.minecraft.world.level.block.entity.BlockEntity be) {}

        public void addEntity(net.minecraft.world.entity.Entity e) {}

        public net.minecraft.world.level.chunk.status.ChunkStatus getPersistedStatus() {
            return null;
        }

        public void removeBlockEntity(BlockPos pos) {}

        public net.minecraft.nbt.CompoundTag getBlockEntityNbtForSaving(
                BlockPos pos, net.minecraft.core.HolderLookup.Provider p) {
            return null;
        }

        public net.minecraft.world.ticks.TickContainerAccess<net.minecraft.world.level.block.Block>
                getBlockTicks() {
            return null;
        }

        public net.minecraft.world.ticks.TickContainerAccess<net.minecraft.world.level.material.Fluid>
                getFluidTicks() {
            return null;
        }

        public ChunkAccess.PackedTicks getTicksForSerialization(long gameTime) {
            return null;
        }
    }

    static BlockPos loadCave(String path, Map<BlockPos, BlockState> blocks) {
        BlockPos origin = new BlockPos(98, -43, -23);
        try (var br = java.nio.file.Files.newBufferedReader(java.nio.file.Path.of(path))) {
            String line;
            while ((line = br.readLine()) != null) {
                line = line.trim();
                if (line.isEmpty() || line.startsWith("#")) continue;
                String[] p = line.split("\\s+");
                if (p[0].equals("origin")) {
                    origin = new BlockPos(Integer.parseInt(p[1]), Integer.parseInt(p[2]), Integer.parseInt(p[3]));
                    continue;
                }
                int x = Integer.parseInt(p[0]);
                int y = Integer.parseInt(p[1]);
                int z = Integer.parseInt(p[2]);
                blocks.put(new BlockPos(x, y, z), named(p[3]));
            }
        } catch (Exception e) {
            throw new RuntimeException(e);
        }
        return origin;
    }

    static BlockState named(String n) {
        return switch (n) {
            case "deepslate" -> Blocks.DEEPSLATE.defaultBlockState();
            case "stone" -> Blocks.STONE.defaultBlockState();
            case "tuff" -> Blocks.TUFF.defaultBlockState();
            case "granite" -> Blocks.GRANITE.defaultBlockState();
            case "diorite" -> Blocks.DIORITE.defaultBlockState();
            case "andesite" -> Blocks.ANDESITE.defaultBlockState();
            case "dirt" -> Blocks.DIRT.defaultBlockState();
            case "gravel" -> Blocks.GRAVEL.defaultBlockState();
            case "calcite" -> Blocks.CALCITE.defaultBlockState();
            case "clay" -> Blocks.CLAY.defaultBlockState();
            case "sand" -> Blocks.SAND.defaultBlockState();
            case "water" -> Blocks.WATER.defaultBlockState();
            case "lava" -> Blocks.LAVA.defaultBlockState();
            case "coal_ore" -> Blocks.COAL_ORE.defaultBlockState();
            case "iron_ore" -> Blocks.IRON_ORE.defaultBlockState();
            case "copper_ore" -> Blocks.COPPER_ORE.defaultBlockState();
            case "gold_ore" -> Blocks.GOLD_ORE.defaultBlockState();
            case "redstone_ore" -> Blocks.REDSTONE_ORE.defaultBlockState();
            case "lapis_ore" -> Blocks.LAPIS_ORE.defaultBlockState();
            case "diamond_ore" -> Blocks.DIAMOND_ORE.defaultBlockState();
            case "deepslate_coal_ore" -> Blocks.DEEPSLATE_COAL_ORE.defaultBlockState();
            case "deepslate_iron_ore" -> Blocks.DEEPSLATE_IRON_ORE.defaultBlockState();
            case "deepslate_copper_ore" -> Blocks.DEEPSLATE_COPPER_ORE.defaultBlockState();
            case "deepslate_gold_ore" -> Blocks.DEEPSLATE_GOLD_ORE.defaultBlockState();
            case "deepslate_redstone_ore" -> Blocks.DEEPSLATE_REDSTONE_ORE.defaultBlockState();
            case "deepslate_lapis_ore" -> Blocks.DEEPSLATE_LAPIS_ORE.defaultBlockState();
            case "deepslate_diamond_ore" -> Blocks.DEEPSLATE_DIAMOND_ORE.defaultBlockState();
            case "sculk_vein" -> Blocks.SCULK_VEIN.defaultBlockState();
            case "sculk" -> Blocks.SCULK.defaultBlockState();
            case "air" -> Blocks.AIR.defaultBlockState();
            case "bedrock" -> Blocks.BEDROCK.defaultBlockState();
            case "raw_iron_block" -> Blocks.RAW_IRON_BLOCK.defaultBlockState();
            default -> Blocks.STONE.defaultBlockState();
        };
    }

    @SuppressWarnings("unchecked")
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

    static LevelAccessor fakeLevel(Map<BlockPos, BlockState> blocks, ChunkAccess chunk) {
        return (LevelAccessor)
                Proxy.newProxyInstance(
                        LevelAccessor.class.getClassLoader(),
                        new Class<?>[] {LevelAccessor.class},
                        (proxy, method, args) -> {
                            String n = method.getName();
                            Class<?> ret = method.getReturnType();
                            switch (n) {
                                case "getBlockState" -> {
                                    BlockPos p = (BlockPos) args[0];
                                    return blocks.getOrDefault(p.immutable(), Blocks.AIR.defaultBlockState());
                                }
                                case "setBlock" -> {
                                    BlockPos p = ((BlockPos) args[0]).immutable();
                                    BlockState s = (BlockState) args[1];
                                    blocks.put(p, s);
                                    return true;
                                }
                                case "getFluidState" -> {
                                    BlockPos p = (BlockPos) args[0];
                                    BlockState s =
                                            blocks.getOrDefault(p.immutable(), Blocks.AIR.defaultBlockState());
                                    FluidState f = s.getFluidState();
                                    return f != null ? f : Fluids.EMPTY.defaultFluidState();
                                }
                                case "playSound" -> {
                                    return null;
                                }
                                case "getChunk" -> {
                                    return chunk;
                                }
                                case "getHeight" -> {
                                    if (args != null && args.length >= 3) return 10;
                                    return 384;
                                }
                                case "getMinY" -> {
                                    return -64;
                                }
                                case "getMaxY" -> {
                                    return 319;
                                }
                                case "getMinSectionY" -> {
                                    return -4;
                                }
                                case "getSectionsCount" -> {
                                    return 24;
                                }
                                case "isClientSide" -> {
                                    return false;
                                }
                                case "isEmptyBlock" -> {
                                    BlockPos p = (BlockPos) args[0];
                                    return blocks.getOrDefault(p.immutable(), Blocks.AIR.defaultBlockState())
                                            .isAir();
                                }
                                case "getBlockTicks", "getFluidTicks" -> {
                                    return null;
                                }
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
