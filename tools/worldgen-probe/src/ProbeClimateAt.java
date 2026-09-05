import net.minecraft.core.Holder;
import net.minecraft.data.registries.VanillaRegistries;
import net.minecraft.core.registries.Registries;
import net.minecraft.resources.ResourceKey;
import net.minecraft.resources.Identifier;
import net.minecraft.server.Bootstrap;
import net.minecraft.util.RandomSource;
import net.minecraft.world.level.biome.Biome;
import net.minecraft.world.level.biome.BiomeManager;
import net.minecraft.world.level.biome.Climate;
import net.minecraft.world.level.biome.MultiNoiseBiomeSource;
import net.minecraft.world.level.block.Blocks;
import net.minecraft.world.level.levelgen.NoiseGeneratorSettings;
import net.minecraft.world.level.levelgen.RandomState;
import net.minecraft.world.level.levelgen.SurfaceRules;

public class ProbeClimateAt {
    public static void main(String[] args) {
        long seed = Long.parseLong(args[0]);
        net.minecraft.SharedConstants.tryDetectVersion();
        Bootstrap.bootStrap();
        var lookup = VanillaRegistries.createLookup();
        var noises = lookup.lookupOrThrow(Registries.NOISE);
        var settings = lookup.lookupOrThrow(Registries.NOISE_SETTINGS).getOrThrow(NoiseGeneratorSettings.OVERWORLD);
        RandomState rs = RandomState.create(settings.value(), noises, seed);
        var registry = lookup.lookupOrThrow(Registries.MULTI_NOISE_BIOME_SOURCE_PARAMETER_LIST);
        var key = ResourceKey.create(Registries.MULTI_NOISE_BIOME_SOURCE_PARAMETER_LIST, Identifier.parse("minecraft:overworld"));
        var source = MultiNoiseBiomeSource.createFromPreset(registry.getOrThrow(key));
        var sampler = rs.sampler();
        // climate sample per block coord
        for (int i = 1; i + 2 < args.length; i += 3) {
            int x = Integer.parseInt(args[i]);
            int y = Integer.parseInt(args[i + 1]);
            int z = Integer.parseInt(args[i + 2]);
            Climate.TargetPoint tp = sampler.sample(x, y, z);
            System.out.println("CLIMATE " + x + "," + y + "," + z
                + " temp=" + tp.temperature()
                + " humid=" + tp.humidity()
                + " cont=" + tp.continentalness()
                + " erosion=" + tp.erosion()
                + " depth=" + tp.depth()
                + " weird=" + tp.weirdness());
        }
    }
}
