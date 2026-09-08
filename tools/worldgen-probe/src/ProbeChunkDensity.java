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

        // stdin: x y z absolute block coords (chunk derived from x,z)
        java.io.BufferedReader in = new java.io.BufferedReader(new java.io.InputStreamReader(System.in));
        String line;
        while ((line = in.readLine()) != null) {
            line = line.trim();
            if (line.isEmpty()) continue;
            String[] pp = line.split("\\s+");
            int bx = Integer.parseInt(pp[0]);
            int by = Integer.parseInt(pp[1]);
            int bz = Integer.parseInt(pp[2]);
            double d = sampleAbs(rs, settings, beard, fluid, bx, by, bz);
            System.out.println("(" + bx + "," + by + "," + bz + ") interp=" + String.format("%.6f", d)
                + (d > 0 ? " solid" : " air"));
        }
    }

    static double sampleAbs(RandomState rs, Holder<NoiseGeneratorSettings> settings,
                         DensityFunctions.BeardifierOrMarker beard,
                         net.minecraft.world.level.levelgen.Aquifer.FluidPicker fluid,
                         int bx, int by, int bz) throws Exception {
        int cx = Math.floorDiv(bx, 16);
        int cz = Math.floorDiv(bz, 16);
        int lx = bx - cx * 16;
        int lz = bz - cz * 16;
        // re-run sample with local coords, then offset the NoiseChunk to the chunk
        return sampleAt(rs, settings, beard, fluid, cx, cz, lx, by, lz);
    }

    static double sampleAt(RandomState rs, Holder<NoiseGeneratorSettings> settings,
                         DensityFunctions.BeardifierOrMarker beard,
                         net.minecraft.world.level.levelgen.Aquifer.FluidPicker fluid,
                         int cx, int cz, int lx, int by, int lz) throws Exception {
        NoiseSettings ns = settings.value().noiseSettings();
        int cw = ns.getCellWidth(), ch = ns.getCellHeight(), minY = ns.minY();
        int cellCountX = 16 / cw, cellCountZ = 16 / cw, cellCountY = ns.height() / ch;
        int cellXM = lx / cw, cellZM = lz / cw, cellYM = (by - minY) / ch;
        int xicT = lx % cw, yicT = (by - minY) % ch, zicT = lz % cw;
        var nc = new NC(4, rs, cx * 16, cz * 16, ns, beard, settings.value(), fluid, Blender.empty());
        nc.initializeForFirstCellX();
        double result = Double.NaN;
        for (int ccx = 0; ccx < cellCountX; ccx++) {
            nc.advanceCellX(ccx);
            for (int ccz = 0; ccz < cellCountZ; ccz++) {
                for (int cy = cellCountY - 1; cy >= 0; cy--) {
                    nc.selectCellYZ(cy, ccz);
                    for (int yic = ch - 1; yic >= 0; yic--) {
                        int posY = (minY / ch + cy) * ch + yic;
                        nc.updateForY(posY, (double) yic / ch);
                        for (int xic = 0; xic < cw; xic++) {
                            int posX = ccx * cw + xic;
                            if (posX == lx && posY == by && ccz * cw + zicT == lz) {
                                result = nc.interp();
                            }
                        }
                    }
                }
            }
        }
        return result;
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
