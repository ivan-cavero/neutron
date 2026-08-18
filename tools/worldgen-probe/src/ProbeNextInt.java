import net.minecraft.world.level.levelgen.WorldgenRandom;
import net.minecraft.world.level.levelgen.XoroshiroRandomSource;

public class ProbeNextInt {
    public static void main(String[] args) {
        WorldgenRandom rng = new WorldgenRandom(new XoroshiroRandomSource(0));
        long dec = rng.setDecorationSeed(424242L, 0, 0);
        rng.setFeatureSeed(dec, 29, 9);
        // 2x nextInt(16) then nextInt(321), 3 positions
        for (int i = 0; i < 3; i++) {
            int x = rng.nextInt(16);
            int z = rng.nextInt(16);
            int y = -64 + rng.nextInt(321);
            System.out.println("pos " + (i+1) + " x=" + x + " z=" + z + " y=" + y);
        }
    }
}
