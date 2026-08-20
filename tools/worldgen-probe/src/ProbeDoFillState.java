import java.util.Locale;
import net.minecraft.SharedConstants;
import net.minecraft.core.Holder;
import net.minecraft.core.HolderGetter;
import net.minecraft.core.registries.Registries;
import net.minecraft.data.registries.VanillaRegistries;
import net.minecraft.server.Bootstrap;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.levelgen.DensityFunctions;
import net.minecraft.world.level.levelgen.NoiseChunk;
import net.minecraft.world.level.levelgen.NoiseGeneratorSettings;
import net.minecraft.world.level.levelgen.NoiseSettings;
import net.minecraft.world.level.levelgen.RandomState;
import net.minecraft.world.level.levelgen.blending.Blender;
import net.minecraft.world.level.levelgen.synth.NormalNoise;

/** doFill-like NoiseChunk walk; density + getInterpolatedState at given cells.
 *  Beardifier = Marker (limitation). Chunk origin = chunk containing the cell. */
public class ProbeDoFillState {
    static class NC extends NoiseChunk {
        NC(int cellXZ, RandomState rs, int x, int z, NoiseSettings ns,
           DensityFunctions.BeardifierOrMarker beard, NoiseGeneratorSettings set,
           net.minecraft.world.level.levelgen.Aquifer.FluidPicker fluid, Blender b) {
            super(cellXZ, rs, x, z, ns, beard, set, fluid, b);
        }
        double interp() { return this.getInterpolatedDensity(); }
        BlockState state() { return this.getInterpolatedState(); }
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

        System.out.println("limitation=BeardifierMarker (not structure Beardifier)");
        int[][] pts = {
            {12,1,15},{10,2,15},{8,3,14},{2,5,14},{5,5,14},{1,5,15},{8,3,15},{2,5,15},{5,5,15},
            {1,6,21},{3,6,23}
        };
        for (int[] p : pts) {
            int bx = p[0], by = p[1], bz = p[2];
            int cx = Math.floorDiv(bx, 16), cz = Math.floorDiv(bz, 16);
            Sample s = sample(rs, settings, beard, fluid, cx, cz, bx, by, bz);
            String block = s.state == null ? "null" : String.valueOf(
                net.minecraft.core.registries.BuiltInRegistries.BLOCK.getKey(s.state.getBlock()));
            System.out.printf(Locale.ROOT, "%d %d %d chunk=(%d,%d) density=%+.6f %s block=%s%n",
                bx, by, bz, cx, cz, s.density, s.density > 0 ? "solid" : "open", block);
        }
    }

    static class Sample { double density; BlockState state; }

    static Sample sample(RandomState rs, Holder<NoiseGeneratorSettings> settings,
                         DensityFunctions.BeardifierOrMarker beard,
                         net.minecraft.world.level.levelgen.Aquifer.FluidPicker fluid,
                         int chunkX, int chunkZ, int bx, int by, int bz) {
        NoiseSettings ns = settings.value().noiseSettings();
        int cw = ns.getCellWidth(), ch = ns.getCellHeight(), minY = ns.minY();
        int cellCountX = 16 / cw, cellCountZ = 16 / cw, cellCountY = ns.height() / ch;
        int lx = Math.floorMod(bx, 16), lz = Math.floorMod(bz, 16);
        int cellXM = lx / cw, cellZM = lz / cw, cellYM = (by - minY) / ch;
        int xicT = lx % cw, yicT = Math.floorMod(by - minY, ch), zicT = lz % cw;
        var nc = new NC(4, rs, chunkX * 16, chunkZ * 16, ns, beard, settings.value(), fluid, Blender.empty());
        nc.initializeForFirstCellX();
        Sample out = new Sample();
        out.density = Double.NaN;
        for (int cx = 0; cx < cellCountX; cx++) {
            nc.advanceCellX(cx);
            for (int cz = 0; cz < cellCountZ; cz++) {
                for (int cy = cellCountY - 1; cy >= 0; cy--) {
                    nc.selectCellYZ(cy, cz);
                    for (int yic = ch - 1; yic >= 0; yic--) {
                        int posY = (minY / ch + cy) * ch + yic;
                        nc.updateForY(posY, (double) yic / ch);
                        for (int xic = 0; xic < cw; xic++) {
                            int posX = chunkX * 16 + cx * cw + xic;
                            nc.updateForX(posX, (double) xic / cw);
                            for (int zic = 0; zic < cw; zic++) {
                                int posZ = chunkZ * 16 + cz * cw + zic;
                                nc.updateForZ(posZ, (double) zic / cw);
                                if (cx == cellXM && cz == cellZM && cy == cellYM
                                    && xic == xicT && yic == yicT && zic == zicT) {
                                    out.density = nc.interp();
                                    out.state = nc.state();
                                }
                            }
                        }
                    }
                }
            }
            nc.swapSlices();
        }
        return out;
    }
}
