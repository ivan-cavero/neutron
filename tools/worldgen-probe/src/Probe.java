import net.minecraft.world.level.levelgen.RandomSupport;
import net.minecraft.world.level.levelgen.XoroshiroRandomSource;
import net.minecraft.util.RandomSource;

public class Probe {
    public static void main(String[] args) {
        long seed = Long.parseLong(args[0]);
        RandomSource rng = new XoroshiroRandomSource(seed);
        for (int i = 0; i < 8; i++) {
            System.out.println("rng[" + i + "]=" + rng.nextDouble());
        }
    }
}
