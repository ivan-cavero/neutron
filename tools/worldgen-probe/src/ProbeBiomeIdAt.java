import net.minecraft.SharedConstants;
import net.minecraft.core.Holder;
import net.minecraft.core.registries.Registries;
import net.minecraft.data.registries.VanillaRegistries;
import net.minecraft.resources.Identifier;
import net.minecraft.server.Bootstrap;
import net.minecraft.world.level.biome.Biome;
import net.minecraft.world.level.biome.BiomeManager;
import net.minecraft.world.level.levelgen.NoiseGeneratorSettings;
import net.minecraft.world.level.levelgen.RandomState;
import net.minecraft.world.level.biome.Climate;
import net.minecraft.world.level.levelgen.DensityFunction;
import java.io.BufferedReader;
import java.io.InputStreamReader;

public class ProbeBiomeIdAt {
    public static void main(String[] args) throws Exception {
        long seed = Long.parseLong(args[0]);
        SharedConstants.tryDetectVersion();
        Bootstrap.bootStrap();
        var lookup = VanillaRegistries.createLookup();
        var noises = lookup.lookupOrThrow(Registries.NOISE);
        var settings = lookup.lookupOrThrow(Registries.NOISE_SETTINGS)
            .getOrThrow(NoiseGeneratorSettings.OVERWORLD);
        RandomState rs = RandomState.create(settings.value(), noises, seed);
        var sampler = rs.sampler();
        var biomeSource = net.minecraft.world.level.biome.MultiNoiseBiomeSource
            .createFromPreset(lookup.lookupOrThrow(Registries.MULTI_NOISE_BIOME_SOURCE_PARAMETER_LIST)
                .getOrThrow(net.minecraft.world.level.biome.MultiNoiseBiomeSourceParameterLists.OVERWORLD));
        BufferedReader in = new BufferedReader(new InputStreamReader(System.in));
        String line;
        while ((line = in.readLine()) != null) {
            line = line.trim();
            if (line.isEmpty()) continue;
            String[] p = line.split("\\s+");
            int x = Integer.parseInt(p[0]);
            int y = Integer.parseInt(p[1]);
            int z = Integer.parseInt(p[2]);
            // vanilla MultiNoiseBiomeSource.getNoiseBiome uses QUART coords
            Holder<Biome> b = biomeSource.getNoiseBiome(sampler.sample(x >> 2, y >> 2, z >> 2));
            System.out.println("BIOME " + x + " " + y + " " + z + " " + b.getRegisteredName());
        }
    }
    static class Ctx implements DensityFunction.FunctionContext {
        int x, y, z;
        Ctx(int x, int y, int z) { this.x = x; this.y = y; this.z = z; }
        public int blockX() { return x; }
        public int blockY() { return y; }
        public int blockZ() { return z; }
        public Holder<Biome> getBiome() { throw new UnsupportedOperationException(); }
    }
}
