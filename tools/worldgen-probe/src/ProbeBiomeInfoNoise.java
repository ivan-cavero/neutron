import net.minecraft.SharedConstants;
import net.minecraft.server.Bootstrap;
import net.minecraft.world.level.biome.Biome;

public class ProbeBiomeInfoNoise {
    public static void main(String[] args) {
        SharedConstants.tryDetectVersion();
        Bootstrap.bootStrap();
        // args: x z pairs, sampled as BIOME_INFO_NOISE.getValue(x, z, false)
        for (int i = 0; i + 1 < args.length; i += 2) {
            double x = Double.parseDouble(args[i]);
            double z = Double.parseDouble(args[i + 1]);
            System.out.printf("%.9f%n", Biome.BIOME_INFO_NOISE.getValue(x, z, false));
        }
    }
}
