import net.minecraft.SharedConstants;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.server.Bootstrap;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.Blocks;

public class ProbeWallTorchId {
   public static void main(String[] args) {
      SharedConstants.tryDetectVersion();
      Bootstrap.bootStrap();
      Block[] probes = {Blocks.WALL_TORCH, Blocks.TORCH, Blocks.REDSTONE_WALL_TORCH,
                        Blocks.IRON_CHAIN, Blocks.DARK_OAK_FENCE, Blocks.OAK_FENCE};
      for (Block b : probes) {
         System.out.println("ID " + BuiltInRegistries.BLOCK.getKey(b) + " = "
            + BuiltInRegistries.BLOCK.getId(b));
      }
   }
}
