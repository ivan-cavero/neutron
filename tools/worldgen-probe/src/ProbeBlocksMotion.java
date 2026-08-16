import net.minecraft.SharedConstants;
import net.minecraft.server.Bootstrap;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.Blocks;

/** 26.2: Heightmap.OCEAN_FLOOR uses BlockState.blocksMotion(). */
public class ProbeBlocksMotion {
    static void row(String id, Block b) {
        var st = b.defaultBlockState();
        System.out.println(
                id
                        + " blocksMotion="
                        + st.blocksMotion()
                        + " isSolid="
                        + st.isSolid()
                        + " isAir="
                        + st.isAir());
    }

    public static void main(String[] args) {
        SharedConstants.tryDetectVersion();
        Bootstrap.bootStrap();
        row("dark_oak_leaves", Blocks.DARK_OAK_LEAVES);
        row("oak_leaves", Blocks.OAK_LEAVES);
        row("dark_oak_log", Blocks.DARK_OAK_LOG);
        row("grass_block", Blocks.GRASS_BLOCK);
        row("short_grass", Blocks.SHORT_GRASS);
        row("leaf_litter", Blocks.LEAF_LITTER);
        row("snow", Blocks.SNOW);
        row("water", Blocks.WATER);
        row("air", Blocks.AIR);
    }
}
