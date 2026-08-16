import net.minecraft.world.level.levelgen.XoroshiroRandomSource;
import net.minecraft.util.RandomSource;

/** Dump XoroshiroRandomSource sequence for verification against Rust. */
public class ProbeRng {
    public static void main(String[] args) {
        long seed = Long.parseLong(args[0]);
        RandomSource rng = new XoroshiroRandomSource(seed);
        System.out.println("// seed=" + seed);
        for (int i = 0; i < 12; i++) {
            System.out.printf("nextDouble[%d] = %.17g%n", i, rng.nextDouble());
        }
        RandomSource rng2 = new XoroshiroRandomSource(seed);
        for (int i = 0; i < 12; i++) {
            System.out.printf("nextInt[%d] = %d%n", i, rng2.nextInt(256));
        }
    }
}
