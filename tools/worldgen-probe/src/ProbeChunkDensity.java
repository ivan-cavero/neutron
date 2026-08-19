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

/** Replicate the real doFill interpolation for chunk (0,0) and read the
 *  interpolated density at the missing-water cells. */
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

        int[][] pts = {{12,1,15},{10,2,15},{8,3,14},{2,5,14},{5,5,14},{1,5,15}};
        for (int[] p : pts) {
            double d = sample(rs, settings, beard, fluid, p[0], p[1], p[2]);
            System.out.println("(" + p[0] + "," + p[1] + "," + p[2] + ") interp=" + String.format("%.6f", d)
                + (d > 0 ? " solid" : " air"));
        }
    }

    static double sample(RandomState rs, Holder<NoiseGeneratorSettings> settings,
                         DensityFunctions.BeardifierOrMarker beard,
                         net.minecraft.world.level.levelgen.Aquifer.FluidPicker fluid,
                         int bx, int by, int bz) throws Exception {
        NoiseSettings ns = settings.value().noiseSettings();
        int cw = ns.getCellWidth(), ch = ns.getCellHeight(), minY = ns.minY();
        int cellCountX = 16 / cw, cellCountZ = 16 / cw, cellCountY = ns.height() / ch;
        int cellXM = (bx % 16) / cw, cellZM = (bz % 16) / cw, cellYM = (by - minY) / ch;
        int xicT = bx % cw, yicT = (by - minY) % ch, zicT = bz % cw;
        var nc = new NC(4, rs, 0, 0, ns, beard, settings.value(), fluid, Blender.empty());
        nc.initializeForFirstCellX();
        double result = Double.NaN;
        for (int cx = 0; cx < cellCountX; cx++) {
            nc.advanceCellX(cx);
            for (int cz = 0; cz < cellCountZ; cz++) {
                for (int cy = cellCountY - 1; cy >= 0; cy--) {
                    nc.selectCellYZ(cy, cz);
                    for (int yic = ch - 1; yic >= 0; yic--) {
                        int posY = (minY / ch + cy) * ch + yic;
                        nc.updateForY(posY, (double) yic / ch);
                        for (int xic = 0; xic < cw; xic++) {
                            int posX = cx * cw + xic;
                            nc.updateForX(posX, (double) xic / cw);
                            for (int zic = 0; zic < cw; zic++) {
                                int posZ = cz * cw + zic;
                                nc.updateForZ(posZ, (double) zic / cw);
                                if (cx == cellXM && cz == cellZM && cy == cellYM
                                    && xic == xicT && yic == yicT && zic == zicT) {
                                    result = nc.interp();
                                }
                            }
                        }
                    }
                }
            }
            nc.swapSlices();
        }
        return result;
    }
}
