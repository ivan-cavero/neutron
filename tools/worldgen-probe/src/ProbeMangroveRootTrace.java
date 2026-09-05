import net.minecraft.core.BlockPos;
import net.minecraft.core.Direction;
import net.minecraft.util.RandomSource;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.levelgen.feature.TreeFeature;

/**
 * Replays the MangroveRootPlacer root walk against the ProbeDecorate scene,
 * logging every potentialRootPositions call (position, width, branch taken,
 * canPlaceRoot result) so the vanilla walk can be diffed against neutron's
 * NEUTRON_ROOT_TRACE. Run with the same args as ProbeFullDecorate after it
 * (the scene must be decorated up to the tree).
 *
 * Usage: ProbeMangroveRootTrace <x> <y> <z> <dirIdx> <maxRootWidth>
 *        <maxRootLength> <skew> <drawsCsv>
 * where drawsCsv is the vanilla post-offset draw list (floats and bools in
 * order). Emits ROOT lines in lockstep with the draws.
 */
public class ProbeMangroveRootTrace {
    static java.util.ArrayDeque<Object> draws = new java.util.ArrayDeque<>();
    static int calls = 0;

    public static void main(String[] args) throws Exception {
        net.minecraft.SharedConstants.tryDetectVersion();
        net.minecraft.server.Bootstrap.bootStrap();
        int x = Integer.parseInt(args[0]);
        int y = Integer.parseInt(args[1]);
        int z = Integer.parseInt(args[2]);
        int dirIdx = Integer.parseInt(args[3]);
        int maxW = Integer.parseInt(args[4]);
        int maxL = Integer.parseInt(args[5]);
        float skew = Float.parseFloat(args[6]);
        for (String t : args[7].split(",")) {
            t = t.trim();
            if (t.equals("true")) draws.add(Boolean.TRUE);
            else if (t.equals("false")) draws.add(Boolean.FALSE);
            else draws.add(Float.parseFloat(t));
        }
        Direction dir = new Direction[]{Direction.NORTH, Direction.EAST, Direction.SOUTH, Direction.WEST}[dirIdx & 3];
        // trunkOrigin-relative: origin y passed in is trunkOrigin.y
        BlockPos pos = new BlockPos(x, y, z);
        java.util.List<BlockPos> positions = new java.util.ArrayList<>();
        boolean ok = simulate(pos, dir, new BlockPos(x, y, z), positions, 0, maxL, maxW, skew);
        System.out.println("ROOTWALK ok=" + ok + " positions=" + positions.size() + " calls=" + calls);
        for (BlockPos p : positions) {
            System.out.println("ROOTPOS " + p.getX() + "," + p.getY() + "," + p.getZ());
        }
    }

    static boolean simulate(BlockPos rootPos, Direction prevDir, BlockPos rootOrigin,
                            java.util.List<BlockPos> positions, int layer,
                            int maxL, int maxW, float skew) {
        if (layer != maxL && positions.size() <= maxL) {
            for (BlockPos pos : potential(rootPos, prevDir, rootOrigin, maxW, skew)) {
                calls++;
                if (canPlace(pos)) {
                    positions.add(pos);
                    System.out.println("ROOTCALL " + calls + " pos=" + pos.getX() + "," + pos.getY() + "," + pos.getZ() + " canPlace=true");
                    if (!simulate(pos, prevDir, rootOrigin, positions, layer + 1, maxL, maxW, skew)) {
                        return false;
                    }
                } else {
                    System.out.println("ROOTCALL " + calls + " pos=" + pos.getX() + "," + pos.getY() + "," + pos.getZ() + " canPlace=false");
                }
            }
            return true;
        }
        return false;
    }

    static java.util.List<BlockPos> potential(BlockPos pos, Direction prevDir,
                                              BlockPos rootOrigin, int maxW, float skew) {
        BlockPos below = pos.below();
        BlockPos nextTo = pos.relative(prevDir);
        int width = pos.distManhattan(rootOrigin);
        if (width > maxW - 3 && width <= maxW) {
            float f = nextFloat();
            System.out.println("ROOTDRAW band f=" + f + " width=" + width);
            return f < skew ? java.util.List.of(below, nextTo.below()) : java.util.List.of(below);
        } else if (width > maxW) {
            System.out.println("ROOTDRAW over width=" + width);
            return java.util.List.of(below);
        } else if (nextFloat() < skew) {
            System.out.println("ROOTDRAW else-skew");
            return java.util.List.of(below);
        } else {
            boolean b = nextBoolean();
            System.out.println("ROOTDRAW else-bool b=" + b);
            return b ? java.util.List.of(nextTo) : java.util.List.of(below);
        }
    }

    static float nextFloat() {
        Object d = draws.poll();
        if (d instanceof Float f) return f;
        throw new IllegalStateException("draw underrun: expected float got " + d);
    }

    static boolean nextBoolean() {
        Object d = draws.poll();
        if (d instanceof Boolean b) return b;
        throw new IllegalStateException("draw underrun: expected bool got " + d);
    }

    static boolean canPlace(BlockPos pos) {
        // MangroveRootPlacer.canPlaceRoot = TreeFeature.validTreePos || canGrowThrough
        net.minecraft.world.level.block.state.BlockState st =
            ProbeDecorate.getState(pos.getX(), pos.getY(), pos.getZ());
        return st.isAir()
            || st.is(net.minecraft.tags.BlockTags.REPLACEABLE_BY_TREES)
            || st.is(net.minecraft.tags.BlockTags.MANGROVE_ROOTS_CAN_GROW_THROUGH);
    }
}
