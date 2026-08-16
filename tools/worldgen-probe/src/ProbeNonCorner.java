import net.minecraft.core.BlockPos;
import net.minecraft.core.Vec3i;

/** ChargeCursor.NON_CORNER_NEIGHBOURS construction order. */
public class ProbeNonCorner {
    public static void main(String[] args) {
        BlockPos.betweenClosedStream(new BlockPos(-1, -1, -1), new BlockPos(1, 1, 1))
                .filter(
                        p ->
                                !((p.getX() != 0 && p.getY() != 0 && p.getZ() != 0)
                                        || p.equals(BlockPos.ZERO)))
                .map(p -> new Vec3i(p.getX(), p.getY(), p.getZ()))
                .forEach(v -> System.out.println(v.getX() + "," + v.getY() + "," + v.getZ()));
    }
}
