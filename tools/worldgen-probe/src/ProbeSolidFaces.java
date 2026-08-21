import net.minecraft.SharedConstants;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Direction;
import net.minecraft.server.Bootstrap;
import net.minecraft.world.level.EmptyBlockGetter;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.Blocks;
import net.minecraft.world.level.block.state.BlockState;

/** 26.2: isSolid vs blocksMotion vs isFaceSturdy for vegetation_patch / environment_scan. */
public class ProbeSolidFaces {
    static void row(String id, Block b) {
        BlockState st = b.defaultBlockState();
        boolean up = st.isFaceSturdy(EmptyBlockGetter.INSTANCE, BlockPos.ZERO, Direction.UP);
        boolean down = st.isFaceSturdy(EmptyBlockGetter.INSTANCE, BlockPos.ZERO, Direction.DOWN);
        System.out.println(
                id
                        + " blocksMotion="
                        + st.blocksMotion()
                        + " isSolid="
                        + st.isSolid()
                        + " isAir="
                        + st.isAir()
                        + " faceSturdyUP="
                        + up
                        + " faceSturdyDOWN="
                        + down);
    }

    public static void main(String[] args) {
        SharedConstants.tryDetectVersion();
        Bootstrap.bootStrap();
        row("stone", Blocks.STONE);
        row("deepslate", Blocks.DEEPSLATE);
        row("tuff", Blocks.TUFF);
        row("calcite", Blocks.CALCITE);
        row("clay", Blocks.CLAY);
        row("gravel", Blocks.GRAVEL);
        row("dirt", Blocks.DIRT);
        row("grass_block", Blocks.GRASS_BLOCK);
        row("moss_block", Blocks.MOSS_BLOCK);
        row("moss_carpet", Blocks.MOSS_CARPET);
        row("glow_lichen", Blocks.GLOW_LICHEN);
        row("sculk_vein", Blocks.SCULK_VEIN);
        row("sculk", Blocks.SCULK);
        row("vine", Blocks.VINE);
        row("cave_vines", Blocks.CAVE_VINES);
        row("pointed_dripstone", Blocks.POINTED_DRIPSTONE);
        row("dripstone_block", Blocks.DRIPSTONE_BLOCK);
        row("ice", Blocks.ICE);
        row("packed_ice", Blocks.PACKED_ICE);
        row("blue_ice", Blocks.BLUE_ICE);
        row("oak_leaves", Blocks.OAK_LEAVES);
        row("azalea", Blocks.AZALEA);
        row("flowering_azalea", Blocks.FLOWERING_AZALEA);
        row("spore_blossom", Blocks.SPORE_BLOSSOM);
        row("hanging_roots", Blocks.HANGING_ROOTS);
        row("water", Blocks.WATER);
        row("lava", Blocks.LAVA);
        row("air", Blocks.AIR);
        row("cave_air", Blocks.CAVE_AIR);
        row("snow", Blocks.SNOW);
        row("short_grass", Blocks.SHORT_GRASS);
        row("bamboo", Blocks.BAMBOO);
        row("pumpkin", Blocks.PUMPKIN);
        row("cobblestone", Blocks.COBBLESTONE);
        row("rooted_dirt", Blocks.ROOTED_DIRT);
        row("pale_moss_block", Blocks.PALE_MOSS_BLOCK);
        row("pale_moss_carpet", Blocks.PALE_MOSS_CARPET);
        row("pale_hanging_moss", Blocks.PALE_HANGING_MOSS);
    }
}
