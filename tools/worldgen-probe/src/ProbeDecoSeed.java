import net.minecraft.SharedConstants;
import net.minecraft.server.Bootstrap;
import net.minecraft.world.level.levelgen.WorldgenRandom;
import net.minecraft.world.level.levelgen.XoroshiroRandomSource;

public class ProbeDecoSeed {
    public static void main(String[] args) {
        long seed = Long.parseLong(args[0]);
        int cx = Integer.parseInt(args[1]);
        int cz = Integer.parseInt(args[2]);
        SharedConstants.tryDetectVersion();
        Bootstrap.bootStrap();
        WorldgenRandom r = new WorldgenRandom(new XoroshiroRandomSource(
            net.minecraft.world.level.levelgen.RandomSupport.generateUniqueSeed()));
        long decorationSeed = r.setDecorationSeed(seed, cx * 16, cz * 16);
        System.out.println("decorationSeed=" + decorationSeed);
        r.setFeatureSeed(decorationSeed, 4, 7);
        System.out.println("feature4_step7_first_int=" + r.nextInt(49) + " then " + r.nextInt(16)
            + "," + r.nextInt(16) + "," + r.nextInt(257));
    }
}
