import net.minecraft.SharedConstants;
import net.minecraft.core.HolderGetter;
import net.minecraft.core.registries.Registries;
import net.minecraft.data.registries.VanillaRegistries;
import net.minecraft.resources.Identifier;
import net.minecraft.resources.ResourceKey;
import net.minecraft.server.Bootstrap;
import net.minecraft.world.level.levelgen.DensityFunction;
import net.minecraft.world.level.levelgen.DensityFunctions;
import net.minecraft.world.level.levelgen.NoiseGeneratorSettings;
import net.minecraft.world.level.levelgen.RandomState;
import net.minecraft.world.level.levelgen.synth.NormalNoise;

public class ProbeDepth3 {
    public static void main(String[] args) throws Exception {
        long seed = Long.parseLong(args[0]);
        SharedConstants.tryDetectVersion();
        Bootstrap.bootStrap();
        var lookup = VanillaRegistries.createLookup();
        HolderGetter<NormalNoise.NoiseParameters> noises = lookup.lookupOrThrow(Registries.NOISE);
        var densityFunctions = lookup.lookupOrThrow(Registries.DENSITY_FUNCTION);
        var settings = lookup.lookupOrThrow(Registries.NOISE_SETTINGS).getOrThrow(NoiseGeneratorSettings.OVERWORLD);
        RandomState rs = RandomState.create(settings.value(), noises, seed);
        // Resolve the depth holder to the real function (raw, unwired).
        var depthKey = ResourceKey.create(Registries.DENSITY_FUNCTION, Identifier.parse("minecraft:overworld/depth"));
        DensityFunction rawDepth = densityFunctions.getOrThrow(depthKey).value();
        DensityFunction wired = rs.router().depth();
        DensityFunction wiredRaw = resolveHolder(wired, densityFunctions);
        for (int y = 76; y <= 96; y += 4) {
            double w = wired.compute(new DensityFunction.SinglePointContext(4, y, 4));
            double wr = wiredRaw.compute(new DensityFunction.SinglePointContext(4, y, 4));
            double r = rawDepth.compute(new DensityFunction.SinglePointContext(4, y, 4));
            System.out.println("y=" + y + " wired=" + String.format("%.4f", w) + " wiredRaw=" + String.format("%.4f", wr) + " raw=" + String.format("%.4f", r));
        }
    }
    static DensityFunction resolveHolder(DensityFunction f, HolderGetter<DensityFunction> reg) {
        if (f instanceof DensityFunctions.HolderHolder h) {
            return h.function().value();
        }
        return f;
    }
}
