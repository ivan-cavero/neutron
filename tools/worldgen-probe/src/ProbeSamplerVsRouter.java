import net.minecraft.SharedConstants;
import net.minecraft.core.registries.Registries;
import net.minecraft.data.registries.VanillaRegistries;
import net.minecraft.server.Bootstrap;
import net.minecraft.world.level.levelgen.NoiseGeneratorSettings;
import net.minecraft.world.level.levelgen.RandomState;
import net.minecraft.world.level.levelgen.DensityFunction;

public class ProbeSamplerVsRouter {
    public static void main(String[] args) throws Exception {
        long seed = Long.parseLong(args[0]);
        SharedConstants.tryDetectVersion();
        Bootstrap.bootStrap();
        var lookup = VanillaRegistries.createLookup();
        var noises = lookup.lookupOrThrow(Registries.NOISE);
        var settings = lookup.lookupOrThrow(Registries.NOISE_SETTINGS).getOrThrow(NoiseGeneratorSettings.OVERWORLD);
        RandomState rs = RandomState.create(settings.value(), noises, seed);
        var router = rs.router();
        var sampler = rs.sampler();
        var ctx = new Ctx();
        for (int i = 1; i + 2 < args.length; i += 3) {
            int x = Integer.parseInt(args[i]);
            int y = Integer.parseInt(args[i + 1]);
            int z = Integer.parseInt(args[i + 2]);
            ctx.set(x, y, z);
            System.out.printf("CMP %d,%d,%d router_temp=%.9f sampler_temp=%.9f router_depth=%.9f sampler_depth=%.9f%n",
                x, y, z,
                router.temperature().compute(ctx),
                sampler.temperature().compute(ctx),
                router.depth().compute(ctx),
                sampler.depth().compute(ctx));
        }
    }
    static class Ctx implements DensityFunction.FunctionContext {
        int x, y, z;
        void set(int x, int y, int z) { this.x = x; this.y = y; this.z = z; }
        public int blockX() { return x; }
        public int blockY() { return y; }
        public int blockZ() { return z; }
        public net.minecraft.core.Holder<net.minecraft.world.level.biome.Biome> getBiome() { throw new UnsupportedOperationException(); }
    }
}
