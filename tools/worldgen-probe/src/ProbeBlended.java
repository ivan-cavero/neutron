import java.util.Locale;
import net.minecraft.world.level.levelgen.synth.BlendedNoise;

/** Dump BlendedNoise.compute values for verification against Rust. */
public class ProbeBlended {
    public static void main(String[] args) {
        net.minecraft.SharedConstants.tryDetectVersion();
        net.minecraft.server.Bootstrap.bootStrap();
        BlendedNoise bn = BlendedNoise.createUnseeded(0.25, 0.125, 80.0, 160.0, 8.0);
        int[][] coords = {{0, 0, 0}, {100, 40, 200}, {-57, 63, 31}, {1234, -64, 5678}, {16, 320, 16}};
        for (int[] c : coords) {
            double v = bn.compute(new net.minecraft.world.level.levelgen.DensityFunction.SinglePointContext(c[0], c[1], c[2]));
            System.out.printf("%s", String.format(Locale.ROOT, "blended(%d,%d,%d) = %.17g%n", c[0], c[1], c[2], v));
        }
        System.out.printf("%s", String.format(Locale.ROOT, "maxValue = %.17g%n", bn.maxValue()));
    }
}
