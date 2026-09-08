import java.util.Locale;
import net.minecraft.SharedConstants;
import net.minecraft.server.Bootstrap;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.Blocks;
import net.minecraft.world.level.block.state.BlockState;

/** Dumps protocol state ids (BLOCK_STATE_REGISTRY) for requested blocks. */
public class ProbeStateIds {
    public static void main(String[] args) {
        SharedConstants.tryDetectVersion();
        Bootstrap.bootStrap();
        for (String name : args) {
            Block b = null;
            for (var f : Blocks.class.getFields()) {
                if (f.getName().equalsIgnoreCase(name.toUpperCase(Locale.ROOT))) {
                    try {
                        b = (Block) f.get(null);
                    } catch (Exception e) {
                        System.out.println(name + " ERR " + e);
                    }
                    break;
                }
            }
            if (b == null) {
                System.out.println(name + " NOT_FOUND");
                continue;
            }
            BlockState st = b.defaultBlockState();
            System.out.println(name + " " + Block.BLOCK_STATE_REGISTRY.getId(st));
        }
    }
}
