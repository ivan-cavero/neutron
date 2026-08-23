import java.util.Locale;
import net.minecraft.core.HolderGetter;
import net.minecraft.core.registries.Registries;
import net.minecraft.data.registries.VanillaRegistries;
import net.minecraft.world.level.levelgen.DensityFunction;
import net.minecraft.world.level.levelgen.NoiseGeneratorSettings;
import net.minecraft.world.level.levelgen.RandomState;
import net.minecraft.world.level.levelgen.synth.NormalNoise;
import net.minecraft.resources.ResourceKey;
import net.minecraft.resources.Identifier;

/** Evaluate overworld vein_toggle / vein_ridged / vein_gap at given coords.
 *  args: seed x,y,z [x,y,z ...] */
public class ProbeVein {
    public static void main(String[] args) throws Exception {
        long seed = Long.parseLong(args[0]);
        net.minecraft.SharedConstants.tryDetectVersion();
        net.minecraft.server.Bootstrap.bootStrap();
        var lookup = VanillaRegistries.createLookup();
        HolderGetter<NormalNoise.NoiseParameters> noises = lookup.lookupOrThrow(Registries.NOISE);
        var settings = lookup.lookupOrThrow(Registries.NOISE_SETTINGS).getOrThrow(NoiseGeneratorSettings.OVERWORLD);
        RandomState rs = RandomState.create(settings.value(), noises, seed);
        var router = rs.router();
        DensityFunction toggle = router.veinToggle();
        DensityFunction ridged = router.veinRidged();
        DensityFunction gap = router.veinGap();
        for (int i = 1; i < args.length; i++) {
            String[] p = args[i].split(",");
            int x = Integer.parseInt(p[0]), y = Integer.parseInt(p[1]), z = Integer.parseInt(p[2]);
            DensityFunction.SinglePointContext ctx = new DensityFunction.SinglePointContext(x, y, z);
            System.out.printf(Locale.ROOT, "(%d,%d,%d) toggle=%.17g ridged=%.17g gap=%.17g%n",
                    x, y, z, toggle.compute(ctx), ridged.compute(ctx), gap.compute(ctx));
        }
    }
}
