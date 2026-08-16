import net.minecraft.world.level.levelgen.WorldgenRandom;
import net.minecraft.world.level.levelgen.XoroshiroRandomSource;

public class ProbeRarity {
    public static void main(String[] args) {
        long seed = 12345L;
        WorldgenRandom rng = new WorldgenRandom(new XoroshiroRandomSource(seed));
        long dec = rng.setDecorationSeed(seed, 96, -32);
        rng.setFeatureSeed(dec, 6, 6);
        float f = rng.nextFloat();
        System.out.println("andesite_upper nextFloat=" + Float.toString(f));
        System.out.println("1/6=" + (1.0f/6.0f));
        System.out.println("vanilla_place=" + (f < 1.0f/6.0f));

        rng = new WorldgenRandom(new XoroshiroRandomSource(seed));
        dec = rng.setDecorationSeed(seed, 96, -32);
        rng.setFeatureSeed(dec, 2, 6); // granite_upper
        f = rng.nextFloat();
        System.out.println("granite_upper nextFloat=" + Float.toString(f) + " place=" + (f < 1.0f/6.0f));

        rng = new WorldgenRandom(new XoroshiroRandomSource(seed));
        dec = rng.setDecorationSeed(seed, 96, -32);
        rng.setFeatureSeed(dec, 4, 6); // diorite_upper
        f = rng.nextFloat();
        System.out.println("diorite_upper nextFloat=" + Float.toString(f) + " place=" + (f < 1.0f/6.0f));
    }
}
