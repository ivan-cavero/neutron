import net.minecraft.SharedConstants;
import net.minecraft.core.Holder;
import net.minecraft.core.HolderGetter;
import net.minecraft.core.registries.Registries;
import net.minecraft.data.registries.VanillaRegistries;
import net.minecraft.server.Bootstrap;
import net.minecraft.world.level.levelgen.DensityFunction;
import net.minecraft.world.level.levelgen.DensityFunctions;
import net.minecraft.world.level.levelgen.NoiseChunk;
import net.minecraft.world.level.levelgen.NoiseGeneratorSettings;
import net.minecraft.world.level.levelgen.NoiseSettings;
import net.minecraft.world.level.levelgen.RandomState;
import net.minecraft.world.level.levelgen.blending.Blender;
import net.minecraft.world.level.levelgen.synth.NormalNoise;

public class ProbeChunkDensity {
    static class NC extends NoiseChunk {
        NC(int cellXZ, RandomState rs, int x, int z, NoiseSettings ns,
           DensityFunctions.BeardifierOrMarker beard, NoiseGeneratorSettings set,
           net.minecraft.world.level.levelgen.Aquifer.FluidPicker fluid, Blender b) {
            super(cellXZ, rs, x, z, ns, beard, set, fluid, b);
        }
        public double interp() { return this.getInterpolatedDensity(); }
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
        var fpField = net.minecraft.world.level.levelgen.NoiseBasedChunkGenerator.class.getDeclaredField("globalFluidPicker");
        fpField.setAccessible(true);
        @SuppressWarnings("unchecked")
        var fluidPicker = (net.minecraft.world.level.levelgen.Aquifer.FluidPicker)
            ((java.util.function.Supplier<?>) fpField.get(gen)).get();
        DensityFunctions.BeardifierOrMarker beard = getBeardifier();
        int[][] pts = {{12,1,15},{10,2,15},{8,3,14},{2,5,14},{5,5,14},{1,5,15}};
        for (int[] p : pts) {
            double d = sample(rs, settings, beard, fluidPicker, p[0], p[1], p[2]);
            System.out.println("(" + p[0] + "," + p[1] + "," + p[2] + ") interp=" + String.format("%.6f", d)
                + (d > 0 ? " solid" : " air"));
        }
    }
    static DensityFunctions.BeardifierOrMarker getBeardifier() throws Exception {
        var cls = Class.forName("net.minecraft.world.level.levelgen.DensityFunctions$BeardifierMarker");
        var f = cls.getField("INSTANCE");
        @SuppressWarnings("unchecked")
        var v = (DensityFunctions.BeardifierOrMarker) f.get(null);
        return v;
    }
    static double sample(RandomState rs, Holder<NoiseGeneratorSettings> settings,
                         DensityFunctions.BeardifierOrMarker beard,
                         net.minecraft.world.level.levelgen.Aquifer.FluidPicker fluid,
                         int bx, int by, int bz) throws Exception {
        NoiseSettings ns = settings.value().noiseSettings();
        int cw = ns.getCellWidth(), ch = ns.getCellHeight(), minY = ns.minY();
        int chunkX = bx - (bx % (cw * 4)), chunkZ = bz - (bz % (cw * 4));
        var nc = new NC(4, rs, chunkX, chunkZ, ns, beard, settings.value(), fluid, Blender.empty());
        nc.initializeForFirstCellX();
        nc.advanceCellX((bx - chunkX) / cw);
        nc.selectCellYZ((by - minY) / ch, (bz - chunkZ) / cw);
        nc.updateForY(by, (double) ((by - minY) % ch) / ch);
        nc.updateForX(bx, (double) ((bx - chunkX) % cw) / cw);
        nc.updateForZ(bz, (double) ((bz - chunkZ) % cw) / cw);
        return nc.interp();
    }
}
