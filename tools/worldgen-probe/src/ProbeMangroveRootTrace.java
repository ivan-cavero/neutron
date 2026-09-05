import net.minecraft.core.BlockPos;
import net.minecraft.core.Direction;
import net.minecraft.world.level.LevelSimulatedReader;
import net.minecraft.world.level.levelgen.feature.TreeFeature;

/**
 * Live-scene mangrove root walk tracer, invoked by ProbeFullDecorate when
 * ROOTWALK=1 and the traced placed feature is a mangrove tree. Logs every
 * potentialRootPositions branch + canPlaceRoot result so the vanilla walk
 * can be diffed against neutron's NEUTRON_ROOT_TRACE position-for-position.
 */
public final class ProbeMangroveRootTrace {
    private ProbeMangroveRootTrace() {}

    /** trunkOffsetY = uniform(min,max) drawn value (the +min already applied). */
    public static void traceWalk(LevelSimulatedReader level, BlockPos featureOrigin,
                                 int offsetY, java.util.List<Object> dice) {
        BlockPos trunkOrigin = featureOrigin.above(offsetY);
        System.out.println("ROOTWALK trunkOrigin=" + trunkOrigin.getX() + ","
            + trunkOrigin.getY() + "," + trunkOrigin.getZ() + " dice=" + dice.size());
        java.util.ArrayDeque<Object> draws = new java.util.ArrayDeque<>(dice);
        int maxW = 8, maxL = 15;
        float skew = 0.2f;
        for (Direction dir : new Direction[]{Direction.NORTH, Direction.EAST,
                Direction.SOUTH, Direction.WEST}) {
            java.util.List<BlockPos> positions = new java.util.ArrayList<>();
            BlockPos start = trunkOrigin.relative(dir);
            boolean ok = simulate(level, draws, start, dir, trunkOrigin, positions, 0, maxL, maxW, skew);
            System.out.println("ROOTWALK dir=" + dir + " ok=" + ok
                + " positions=" + positions.size());
        }
    }

    static boolean simulate(LevelSimulatedReader level, java.util.ArrayDeque<Object> draws,
                            BlockPos rootPos, Direction prevDir, BlockPos rootOrigin,
                            java.util.List<BlockPos> positions, int layer,
                            int maxL, int maxW, float skew) {
        if (layer != maxL && positions.size() <= maxL) {
            for (BlockPos pos : potential(level, draws, rootPos, prevDir, rootOrigin, maxW, skew)) {
                boolean can = canPlace(level, pos);
                String blk = "";
                if (!can) {
                    blk = " block=" + net.minecraft.core.registries.BuiltInRegistries.BLOCK
                        .getKey(ProbeDecorate.getState(pos.getX(), pos.getY(), pos.getZ()).getBlock())
                        .getPath();
                }
                System.out.println("ROOTCALL pos=" + pos.getX() + "," + pos.getY() + ","
                    + pos.getZ() + " canPlace=" + can + blk);
                if (can) {
                    positions.add(pos);
                    if (!simulate(level, draws, pos, prevDir, rootOrigin, positions,
                            layer + 1, maxL, maxW, skew)) {
                        return false;
                    }
                }
            }
            return true;
        }
        return false;
    }

    static java.util.List<BlockPos> potential(LevelSimulatedReader level,
                                              java.util.ArrayDeque<Object> draws,
                                              BlockPos pos, Direction prevDir,
                                              BlockPos rootOrigin, int maxW, float skew) {
        BlockPos below = pos.below();
        BlockPos nextTo = pos.relative(prevDir);
        int width = pos.distManhattan(rootOrigin);
        if (width > maxW - 3 && width <= maxW) {
            float f = fnext(draws);
            System.out.println("ROOTDRAW band f=" + f + " w=" + width);
            return f < skew ? java.util.List.of(below, nextTo.below())
                            : java.util.List.of(below);
        } else if (width > maxW) {
            System.out.println("ROOTDRAW over w=" + width);
            return java.util.List.of(below);
        } else if (fnext(draws) < skew) {
            System.out.println("ROOTDRAW else-skew");
            return java.util.List.of(below);
        } else {
            boolean b = bnext(draws);
            System.out.println("ROOTDRAW else-bool b=" + b);
            return b ? java.util.List.of(nextTo) : java.util.List.of(below);
        }
    }

    static float fnext(java.util.ArrayDeque<Object> draws) {
        Object d = draws.poll();
        if (d instanceof Float f) return f;
        throw new IllegalStateException("underrun float, got " + d);
    }

    static boolean bnext(java.util.ArrayDeque<Object> draws) {
        Object d = draws.poll();
        if (d instanceof Boolean b) return b;
        throw new IllegalStateException("underrun bool, got " + d);
    }

    static boolean canPlace(LevelSimulatedReader level, BlockPos pos) {
        boolean ok = TreeFeature.validTreePos(level, pos)
            || level.isStateAtPosition(pos,
                st -> st.is(net.minecraft.tags.BlockTags.MANGROVE_ROOTS_CAN_GROW_THROUGH));
        if (!ok) {
            var st = ProbeDecorate.getState(pos.getX(), pos.getY(), pos.getZ());
            System.out.println("ROOTCANFAIL pos=" + pos.getX() + "," + pos.getY() + ","
                + pos.getZ() + " block=" + net.minecraft.core.registries.BuiltInRegistries.BLOCK
                    .getKey(st.getBlock()).getPath());
        }
        return ok;
    }
}
