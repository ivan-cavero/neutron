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
        int[][] pts = {{4,0,0},{0,3,12},{0,3,70},{4,0,17},{11,0,4},{5,15,170},{0,3,22},{4,0,28}};
        for (int[] p : pts) {
            Holder<Biome> b = mgr.getBiome(new BlockPos(p[0], p[1], p[2]));
            System.out.println("(" + p[0] + "," + p[1] + "," + p[2] + ") biome=" + b.unwrapKey().map(k -> k.identifier().toString()).orElse("?"));
        }
    }
    static void HolderGetterHolder() {}
}
