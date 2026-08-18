import net.minecraft.SharedConstants;
import net.minecraft.core.HolderGetter;
import net.minecraft.core.registries.Registries;
import net.minecraft.data.registries.VanillaRegistries;
import net.minecraft.server.Bootstrap;
import net.minecraft.world.level.levelgen.DensityFunction;
import net.minecraft.world.level.levelgen.DensityFunctions;
import net.minecraft.world.level.levelgen.NoiseGeneratorSettings;
import net.minecraft.world.level.levelgen.RandomState;
import net.minecraft.world.level.levelgen.synth.NormalNoise;

public class ProbeDepth2 {
    public static void main(String[] args) throws Exception {
        long seed = Long.parseLong(args[0]);
        SharedConstants.tryDetectVersion();
        Bootstrap.bootStrap();
        var lookup = VanillaRegistries.createLookup();
        HolderGetter<NormalNoise.NoiseParameters> noises = lookup.lookupOrThrow(Registries.NOISE);
        var settings = lookup.lookupOrThrow(Registries.NOISE_SETTINGS).getOrThrow(NoiseGeneratorSettings.OVERWORLD);
        RandomState rs = RandomState.create(settings.value(), noises, seed);
        DensityFunction wired = rs.router().depth();
        DensityFunction raw = settings.value().noiseRouter().depth();
        for (int y = 76; y <= 96; y += 4) {
            double w = wired.compute(new DensityFunction.SinglePointContext(4, y, 4));
            double r = raw.compute(new DensityFunction.SinglePointContext(4, y, 4));
            System.out.println("y=" + y + " wired=" + String.format("%.4f", w) + " raw=" + String.format("%.4f", r));
        }
        System.out.println("wired class=" + wired.getClass().getName());
        System.out.println("raw class=" + raw.getClass().getName());
    }
}
