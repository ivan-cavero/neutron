import net.minecraft.world.level.levelgen.WorldgenRandom;
import net.minecraft.world.level.levelgen.XoroshiroRandomSource;

public class ProbeAndesite {
    public static void main(String[] args) {
        long seed = 12345L;
        WorldgenRandom rng = new WorldgenRandom(new XoroshiroRandomSource(seed));
        long dec = rng.setDecorationSeed(seed, 96, -32);
        rng.setFeatureSeed(dec, 6, 6);
        rng.nextInt(6); rng.nextInt(16); rng.nextInt(16); rng.nextInt(65);
        float ang = rng.nextFloat() * 3.1415927f;
        rng.nextInt(3); rng.nextInt(3);
        System.out.println("angle=" + Float.toString(ang));
        for (int i = 0; i < 8; i++) {
            System.out.println("d[" + i + "]=" + Double.toString(rng.nextDouble()));
        }
    }
}
