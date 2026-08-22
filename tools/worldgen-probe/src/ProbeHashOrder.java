import net.minecraft.core.BlockPos;

import java.nio.file.Files;
import java.nio.file.Path;
import java.util.HashSet;
import java.util.List;
import java.util.Set;

/**
 * Print java.util.HashSet<BlockPos> iteration order for the insertion
 * sequence given as x,y,z lines — ground truth for the Rust
 * tree::java_hash::java_hash_order simulation.
 */
public class ProbeHashOrder {
    public static void main(String[] args) throws Exception {
        List<String> lines = Files.readAllLines(Path.of(args[0]));
        Set<BlockPos> set = new HashSet<>();
        for (String l : lines) {
            l = l.trim();
            if (l.isEmpty()) continue;
            String[] q = l.split(",");
            set.add(new BlockPos(Integer.parseInt(q[0]), Integer.parseInt(q[1]),
                    Integer.parseInt(q[2])));
        }
        StringBuilder sb = new StringBuilder();
        for (BlockPos p : set) {
            sb.append("CELL ").append(p.getX()).append(',').append(p.getY()).append(',')
                    .append(p.getZ()).append('\n');
        }
        System.out.print(sb);
    }
}
