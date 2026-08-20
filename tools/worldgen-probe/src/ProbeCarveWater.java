import net.minecraft.SharedConstants;
import net.minecraft.core.Holder;
import net.minecraft.core.HolderGetter;
import net.minecraft.core.registries.Registries;
import net.minecraft.data.registries.VanillaRegistries;
import net.minecraft.server.Bootstrap;
import net.minecraft.world.level.levelgen.DensityFunctions;
import net.minecraft.world.level.levelgen.NoiseChunk;
import net.minecraft.world.level.levelgen.NoiseGeneratorSettings;
import net.minecraft.world.level.levelgen.NoiseSettings;
import net.minecraft.world.level.levelgen.RandomState;
import net.minecraft.world.level.levelgen.blending.Blender;
import net.minecraft.world.level.levelgen.synth.NormalNoise;

/** Ask the vanilla AQUIFER (density=0, exactly what WorldCarver.getCarveState
 *  does) what block a carved cave cell becomes at the ref's water positions. */
public class ProbeCarveWater {
    static class NC extends NoiseChunk {
        NC(int cellXZ, RandomState rs, int x, int z, NoiseSettings ns,
           DensityFunctions.BeardifierOrMarker beard, NoiseGeneratorSettings set,
           net.minecraft.world.level.levelgen.Aquifer.FluidPicker fluid, Blender b) {
            super(cellXZ, rs, x, z, ns, beard, set, fluid, b);
        }
    }

    public static void main(String[] args) throws Exception {
        long seed = Long.parseLong(args[0]);
        SharedConstants.tryDetectVersion();
        Bootstrap.bootStrap();
        var lookup = VanillaRegistries.createLookup();
        HolderGetter<NormalNoise.NoiseParameters> noises = lookup.lookupOrThrow(Registries.NOISE);
        Holder<NoiseGeneratorSettings> settings =
            lookup.lookupOrThrow(Registries.NOISE_SETTINGS).getOrThrow(NoiseGeneratorSettings.OVERWORLD);
        RandomState rs = RandomState.create(settings.value(), noises, seed);
        var gen = new net.minecraft.world.level.levelgen.NoiseBasedChunkGenerator(
            new net.minecraft.world.level.biome.FixedBiomeSource(
                lookup.lookupOrThrow(Registries.BIOME).getOrThrow(net.minecraft.world.level.biome.Biomes.PLAINS)),
            settings);
        var fp = net.minecraft.world.level.levelgen.NoiseBasedChunkGenerator.class.getDeclaredField("globalFluidPicker");
        fp.setAccessible(true);
        @SuppressWarnings("unchecked")
        var fluid = (net.minecraft.world.level.levelgen.Aquifer.FluidPicker)
            ((java.util.function.Supplier<?>) fp.get(gen)).get();
        var beardCls = Class.forName("net.minecraft.world.level.levelgen.DensityFunctions$BeardifierMarker");
        var bf = beardCls.getField("INSTANCE");
        bf.setAccessible(true);
        @SuppressWarnings("unchecked")
        var beard = (DensityFunctions.BeardifierOrMarker) bf.get(null);

        NoiseSettings ns = settings.value().noiseSettings();
        var nc = new NC(4, rs, 0, 0, ns, beard, settings.value(), fluid, Blender.empty());
        var aquifer = nc.aquifer();

        int[][] pts = {
            {1,5,15},{2,5,14},{2,5,15},{5,5,14},{5,5,15},{8,3,14},{8,3,15},{10,2,15},{12,1,15}
        };
        for (int[] p : pts) {
            var ctx = new net.minecraft.world.level.levelgen.DensityFunctions.SinglePointContext(p[0], p[1], p[2]);
            var water = aquifer.computeSubstance(ctx, 0.0);
            var d = nc.getInterpolatedDensity(); // not meaningful here, placeholder
            System.out.println("(" + p[0] + "," + p[1] + "," + p[2] + ") density0 -> "
                + (water == null ? "null(solid)" : water.getRegisteredName()));
        }
    }
}