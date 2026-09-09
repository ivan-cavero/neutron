import net.minecraft.SharedConstants;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Holder;
import net.minecraft.core.Registry;
import net.minecraft.core.registries.Registries;
import net.minecraft.data.registries.VanillaRegistries;
import net.minecraft.resources.Identifier;
import net.minecraft.resources.ResourceKey;
import net.minecraft.server.Bootstrap;
import net.minecraft.world.level.biome.Biome;
import net.minecraft.world.level.biome.BiomeManager;
import net.minecraft.world.level.biome.Climate;
import net.minecraft.world.level.biome.MultiNoiseBiomeSource;
import net.minecraft.world.level.biome.MultiNoiseBiomeSourceParameterList;
import net.minecraft.world.level.biome.MultiNoiseBiomeSourceParameterLists;
import net.minecraft.world.level.levelgen.NoiseGeneratorSettings;
import net.minecraft.world.level.levelgen.RandomState;
import net.minecraft.world.level.levelgen.synth.NormalNoise;

public class ProbeBiomeAt {
    public static void main(String[] args) throws Exception {
        long seed = Long.parseLong(args[0]);
        SharedConstants.tryDetectVersion();
        Bootstrap.bootStrap();
        var lookup = VanillaRegistries.createLookup();
        HolderGetterHolder(); // noop
        var noises = lookup.lookupOrThrow(Registries.NOISE);
        var settings = lookup.lookupOrThrow(Registries.NOISE_SETTINGS).getOrThrow(NoiseGeneratorSettings.OVERWORLD);
        RandomState rs = RandomState.create(settings.value(), noises, seed);
        var registry = lookup.lookupOrThrow(Registries.MULTI_NOISE_BIOME_SOURCE_PARAMETER_LIST);
        var key = ResourceKey.create(Registries.MULTI_NOISE_BIOME_SOURCE_PARAMETER_LIST, Identifier.parse("minecraft:overworld"));
        var source = MultiNoiseBiomeSource.createFromPreset(registry.getOrThrow(key));
        var sampler = rs.sampler();
        BiomeManager mgr = new BiomeManager(new BiomeManager.NoiseBiomeSource() {
            public Holder<Biome> getNoiseBiome(int qx, int qy, int qz) {
                return source.getNoiseBiome(qx, qy, qz, sampler);
            }
        }, BiomeManager.obfuscateSeed(seed));
        java.io.BufferedReader in = new java.io.BufferedReader(new java.io.InputStreamReader(System.in));
        String line;
        while ((line = in.readLine()) != null) {
            line = line.trim();
            if (line.isEmpty()) continue;
            String[] pp = line.split("\\s+");
            int x = Integer.parseInt(pp[0]), y = Integer.parseInt(pp[1]), z = Integer.parseInt(pp[2]);
            Holder<Biome> b = mgr.getBiome(new BlockPos(x, y, z));
            var tp = sampler.sample(
                net.minecraft.core.QuartPos.fromBlock(x),
                net.minecraft.core.QuartPos.fromBlock(y),
                net.minecraft.core.QuartPos.fromBlock(z));
            // shift_x internals: flat_cache(cache_2d(shift_a(offset))) —
            // evaluate the offset noise at (x*0.25, 0, z*0.25) * 4.
            {
                var offNoise = rs.noiseRegistryValue("minecraft:offset");
                if (offNoise == null) {
                    // fallback: reflect into rs via sampler? Print via the
                    // public NormalNoise if resolvable from the provider.
                    System.out.println("OFFSET unavailable via noiseRegistryValue");
                }
            }
            System.out.println("BIOME " + x + " " + y + " " + z + " " + b.unwrapKey().map(k -> k.identifier().toString()).orElse("?"));
            System.out.printf(java.util.Locale.ROOT,
                "TARGET t=%d h=%d c=%d e=%d d=%d w=%d%n",
                tp.temperature(), tp.humidity(), tp.continentalness(),
                tp.erosion(), tp.depth(), tp.weirdness());
        }
    }
    static void HolderGetterHolder() {}
}
