import net.minecraft.world.level.levelgen.WorldgenRandom;
import net.minecraft.world.level.levelgen.XoroshiroRandomSource;

/** 16 in_square xz for pale_garden_vegetation, chunk (0,0), seed 424242. */
public class ProbePaleDraws {
    public static void main(String[] args) {
        long seed = Long.parseLong(args[0]);
        int ox = Integer.parseInt(args[1]);
        int oz = Integer.parseInt(args[2]);
        int index = Integer.parseInt(args[3]);
        WorldgenRandom rng = new WorldgenRandom(new XoroshiroRandomSource(seed));
        long dec = rng.setDecorationSeed(seed, ox, oz);
        rng.setFeatureSeed(dec, index, 9);
        System.out.println("dec=" + dec + " seed=" + seed + " ox=" + ox + " oz=" + oz + " index=" + index);
        for (int i = 0; i < 16; i++) {
            int x = ox + rng.nextInt(16);
            int z = oz + rng.nextInt(16);
            System.out.println("draw " + (i + 1) + " (" + (x - ox) + "," + (z - oz) + ") abs=(" + x + "," + z + ")");
        }
    }
}
