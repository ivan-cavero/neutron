import net.minecraft.SharedConstants;
import net.minecraft.core.BlockPos;
import net.minecraft.core.registries.Registries;
import net.minecraft.data.registries.VanillaRegistries;
import net.minecraft.server.Bootstrap;
import net.minecraft.world.level.levelgen.NoiseGeneratorSettings;
import net.minecraft.world.level.levelgen.RandomState;
import net.minecraft.world.level.biome.Biome;
import net.minecraft.core.Holder;

public class ProbeBaseNoise {
    public static void main(String[] args) throws Exception {
        long seed = Long.parseLong(args[0]);
        SharedConstants.tryDetectVersion();
        Bootstrap.bootStrap();
        var lookup = VanillaRegistries.createLookup();
        var noises = lookup.lookupOrThrow(Registries.NOISE);
        var settings = lookup.lookupOrThrow(Registries.NOISE_SETTINGS)
            .getOrThrow(NoiseGeneratorSettings.OVERWORLD);
        RandomState rs = RandomState.create(settings.value(), noises, seed);
        var router = rs.router();
        String[] params = {"temperature", "vegetation", "continents", "erosion", "depth", "ridges"};
        var ctx = new DensityFuncContext();
        for (int i = 1; i + 2 < args.length; i += 3) {
            int x = Integer.parseInt(args[i]);
            int y = Integer.parseInt(args[i + 1]);
            int z = Integer.parseInt(args[i + 2]);
            ctx.set(x, y, z);
            StringBuilder sb = new StringBuilder("BASENOISE " + x + "," + y + "," + z);
            for (String p : params) {
                var f = router.getClass().getDeclaredField(p);
                f.setAccessible(true);
                var df = (net.minecraft.world.level.levelgen.DensityFunction) f.get(router);
                sb.append(' ').append(p).append('=').append(df.compute(ctx));
            }
            System.out.println(sb);
        }
    }

    static class DensityFuncContext implements net.minecraft.world.level.levelgen.DensityFunction.FunctionContext {
        int x, y, z;
        void set(int x, int y, int z) { this.x = x; this.y = y; this.z = z; }
        public int blockX() { return x; }
        public int blockY() { return y; }
        public int blockZ() { return z; }
        public Holder<Biome> getBiome() { throw new UnsupportedOperationException(); }
    }
}
