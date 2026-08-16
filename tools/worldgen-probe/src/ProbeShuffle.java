import java.util.ArrayList;
import java.util.List;
import net.minecraft.core.Direction;
import net.minecraft.util.Util;
import net.minecraft.world.level.levelgen.WorldgenRandom;
import net.minecraft.world.level.levelgen.XoroshiroRandomSource;

/** Compare Util.shuffle / Direction.allShuffled to Neutron Fisher-Yates. */
public class ProbeShuffle {
    public static void main(String[] args) {
        WorldgenRandom rng = new WorldgenRandom(new XoroshiroRandomSource(12345L));
        rng.setSeed(12345L);
        List<Integer> a = new ArrayList<>();
        for (int i = 0; i < 18; i++) a.add(i);
        Util.shuffle(a, rng);
        System.out.println("shuffle18=" + a);

        rng = new WorldgenRandom(new XoroshiroRandomSource(12345L));
        rng.setSeed(12345L);
        var dirs = Direction.allShuffled(rng);
        System.out.print("allShuffled=");
        for (Direction d : dirs) System.out.print(d + " ");
        System.out.println();
    }
}
