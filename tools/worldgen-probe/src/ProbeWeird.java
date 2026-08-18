import net.minecraft.SharedConstants;
import net.minecraft.core.HolderGetter;
import net.minecraft.core.registries.Registries;
import net.minecraft.data.registries.VanillaRegistries;
import net.minecraft.resources.Identifier;
import net.minecraft.resources.ResourceKey;
import net.minecraft.server.Bootstrap;
import net.minecraft.world.level.levelgen.DensityFunction;
import net.minecraft.world.level.levelgen.NoiseGeneratorSettings;
import net.minecraft.world.level.levelgen.RandomState;
import net.minecraft.world.level.levelgen.synth.NormalNoise;

public class ProbeWeird {
    public static void main(String[] args) throws Exception {
        long seed = Long.parseLong(args[0]);
        SharedConstants.tryDetectVersion();
        Bootstrap.bootStrap();
        var lookup = VanillaRegistries.createLookup();
        HolderGetter<NormalNoise.NoiseParameters> noises = lookup.lookupOrThrow(Registries.NOISE);
        var settings = lookup.lookupOrThrow(Registries.NOISE_SETTINGS).getOrThrow(NoiseGeneratorSettings.OVERWORLD);
        RandomState rs = RandomState.create(settings.value(), noises, seed);
        DensityFunction samplerWeird = rs.sampler().weirdness();
        var ridgesKey = ResourceKey.create(Registries.DENSITY_FUNCTION, Identifier.parse("minecraft:overworld/ridges"));
        DensityFunction rawRidges = lookup.lookupOrThrow(Registries.DENSITY_FUNCTION).getOrThrow(ridgesKey).value();
        var foldedKey = ResourceKey.create(Registries.DENSITY_FUNCTION, Identifier.parse("minecraft:overworld/ridges_folded"));
        DensityFunction folded = lookup.lookupOrThrow(Registries.DENSITY_FUNCTION).getOrThrow(foldedKey).value();
        for (int y = 76; y <= 88; y += 4) {
            double sw = samplerWeird.compute(new DensityFunction.SinglePointContext(4, y, 4));
            double rr = rawRidges.compute(new DensityFunction.SinglePointContext(4, y, 4));
            double fo = folded.compute(new DensityFunction.SinglePointContext(4, y, 4));
            System.out.println("y=" + y + " samplerWeird=" + String.format("%.4f", sw) + " rawRidges=" + String.format("%.4f", rr) + " folded=" + String.format("%.4f", fo));
        }
    }
}
