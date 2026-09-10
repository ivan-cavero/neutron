import net.minecraft.SharedConstants;
import net.minecraft.server.Bootstrap;
import net.minecraft.world.level.levelgen.WorldgenRandom;
import net.minecraft.world.level.levelgen.XoroshiroRandomSource;

/** Vanilla in_square draws for dark_forest_vegetation (gif 17, step 9) at an origin. */
public class ProbeVegSeed {
    public static void main(String[] args) throws Exception {
        long seed = Long.parseLong(args[0]);
        int ox = Integer.parseInt(args[1]);
        int oz = Integer.parseInt(args[2]);
        int gif = Integer.parseInt(args[3]);
        int step = Integer.parseInt(args[4]);
        SharedConstants.tryDetectVersion();
        Bootstrap.bootStrap();
        WorldgenRandom random = new WorldgenRandom(new XoroshiroRandomSource(0L));
        long decorationSeed = random.setDecorationSeed(seed, ox, oz);
        System.out.println("decorationSeed=" + decorationSeed);
        random.setFeatureSeed(decorationSeed, gif, step);
        for (int i = 0; i < 16; i++) {
            int x = random.nextInt(16) + ox;
            int z = random.nextInt(16) + oz;
            System.out.println("attempt " + i + " x=" + x + " z=" + z);
        }
    }
}
