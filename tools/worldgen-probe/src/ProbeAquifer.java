import net.minecraft.core.Holder;
import net.minecraft.core.HolderGetter;
import net.minecraft.core.QuartPos;
import net.minecraft.core.registries.Registries;
import net.minecraft.data.registries.VanillaRegistries;
import net.minecraft.world.level.LevelHeightAccessor;
import net.minecraft.world.level.levelgen.DensityFunction;
import net.minecraft.world.level.levelgen.NoiseGeneratorSettings;
import net.minecraft.world.level.levelgen.RandomState;
import net.minecraft.world.level.levelgen.synth.NormalNoise;

/**
 * Vanilla preliminarySurfaceLevel noise + Aquifer skipSamplingAboveY for a chunk.
 * Usage: ProbeAquifer <seed> <chunkX> <chunkZ>
 */
public class ProbeAquifer {
    public static void main(String[] args) throws Exception {
        if (args.length > 1 && args[1].equals("substance")) {
            // usage: ProbeAquifer <seed> substance <cells-file>
            String[] rest = new String[args.length - 1];
            rest[0] = args[0];
            System.arraycopy(args, 2, rest, 1, args.length - 2);
            substance(rest);
            return;
        }
        long seed = args.length > 0 ? Long.parseLong(args[0]) : 424242L;
        int cx = args.length > 1 ? Integer.parseInt(args[1]) : 0;
        int cz = args.length > 2 ? Integer.parseInt(args[2]) : 0;
        net.minecraft.SharedConstants.tryDetectVersion();
        net.minecraft.server.Bootstrap.bootStrap();
        var lookup = VanillaRegistries.createLookup();
        HolderGetter<NormalNoise.NoiseParameters> noises = lookup.lookupOrThrow(Registries.NOISE);
        Holder<NoiseGeneratorSettings> settings =
            lookup.lookupOrThrow(Registries.NOISE_SETTINGS).getOrThrow(NoiseGeneratorSettings.OVERWORLD);
        RandomState rs = RandomState.create(settings.value(), noises, seed);
        DensityFunction prelim = rs.router().preliminarySurfaceLevel();

        int minGridX = (cx * 16 - 5) >> 4;
        int maxGridX = ((cx * 16 + 15 - 5) >> 4) + 1;
        int minGridZ = (cz * 16 - 5) >> 4;
        int maxGridZ = ((cz * 16 + 15 - 5) >> 4) + 1;
        int minBlockX = (minGridX << 4) + 0;      // fromGridX(minGridX, 0)
        int maxBlockX = (maxGridX << 4) + 9;      // fromGridX(maxGridX, 9)
        int minBlockZ = (minGridZ << 4) + 0;
        int maxBlockZ = (maxGridZ << 4) + 9;

        int max = Integer.MIN_VALUE;
        System.out.println("x,z -> preliminarySurfaceLevel");
        for (int z = minBlockZ; z <= maxBlockZ; z += 4) {
            for (int x = minBlockX; x <= maxBlockX; x += 4) {
                int qx = QuartPos.toBlock(QuartPos.fromBlock(x));
                int qz = QuartPos.toBlock(QuartPos.fromBlock(z));
                double v = prelim.compute(new DensityFunction.SinglePointContext(qx, 0, qz));
                int lvl = (int) Math.floor(v);
                if (lvl > max) max = lvl;
                System.out.println(x + "," + z + " -> " + lvl);
            }
        }
        int maxAdjusted = max + 8;
        int skipGridY = Math.floorDiv(maxAdjusted + 12, 12) + 1;
        int skipY = skipGridY * 12 + 11 - 1;
        System.out.println("maxPreliminarySurfaceLevel=" + max
            + "  adjustSurfaceLevel=" + maxAdjusted
            + "  skipSamplingAboveGridY=" + skipGridY
            + "  skipSamplingAboveY=" + skipY);
    }

    // substance mode: ProbeAquifer <seed> substance <cells-file>
    // cells-file lines: x y z — prints the aquifer substance block per line.
    static void substance(String[] args) throws Exception {
        long seed = Long.parseLong(args[0]);
        net.minecraft.SharedConstants.tryDetectVersion();
        net.minecraft.server.Bootstrap.bootStrap();
        var lookup = VanillaRegistries.createLookup();
        HolderGetter<NormalNoise.NoiseParameters> noises = lookup.lookupOrThrow(Registries.NOISE);
        Holder<NoiseGeneratorSettings> settingsHolder =
            lookup.lookupOrThrow(Registries.NOISE_SETTINGS).getOrThrow(NoiseGeneratorSettings.OVERWORLD);
        var settings = settingsHolder.value();
        RandomState rs = RandomState.create(settings, noises, seed);
        var biomeSource = new net.minecraft.world.level.biome.FixedBiomeSource(
            lookup.lookupOrThrow(Registries.BIOME).getOrThrow(
                net.minecraft.world.level.biome.Biomes.PLAINS));
        var gen = new net.minecraft.world.level.levelgen.NoiseBasedChunkGenerator(
            biomeSource, settingsHolder);
        java.lang.reflect.Field fp = net.minecraft.world.level.levelgen.NoiseBasedChunkGenerator.class
            .getDeclaredField("globalFluidPicker");
        fp.setAccessible(true);
        var fluid = (net.minecraft.world.level.levelgen.Aquifer.FluidPicker)
            ((java.util.function.Supplier<?>) fp.get(gen)).get();
        Class<?> beardCls = Class.forName(
            "net.minecraft.world.level.levelgen.DensityFunctions$BeardifierMarker");
        var bf = beardCls.getField("INSTANCE");
        bf.setAccessible(true);
        var beard = (net.minecraft.world.level.levelgen.DensityFunctions.BeardifierOrMarker) bf.get(null);

        java.io.BufferedReader in = new java.io.BufferedReader(
            new java.io.InputStreamReader(System.in));
        String line;
        while ((line = in.readLine()) != null) {
            line = line.trim();
            if (line.isEmpty()) continue;
            String[] pp = line.split("\\s+");
            int bx = Integer.parseInt(pp[0]);
            int by = Integer.parseInt(pp[1]);
            int bz = Integer.parseInt(pp[2]);
            int cx = Math.floorDiv(bx, 16);
            int cz = Math.floorDiv(bz, 16);
            LevelHeightAccessor lha = new LevelHeightAccessor() {
                public int getMinY() { return -64; }
                public int getHeight() { return 384; }
                public int getMaxY() { return 320; }
                public boolean isOutsideBuildHeight(int y) { return y < -64 || y >= 320; }
                public int getMinSectionY() { return -4; }
                public int getMaxSectionY() { return 19; }
                public int getSectionsCount() { return 24; }
                public int getSectionIndex(int y) { return (y >> 4) + 4; }
                public int getSectionIndexFromSectionY(int sy) { return sy + 4; }
            };
            var ns = settings.noiseSettings().clampToHeightAccessor(lha);
            var nc = new net.minecraft.world.level.levelgen.NoiseChunk(
                4, rs, cx * 16, cz * 16, ns, beard, settings, fluid,
                net.minecraft.world.level.levelgen.blending.Blender.empty()) {
                @Override
                public double getInterpolatedDensity() {
                    return super.getInterpolatedDensity();
                }
            };
            var aquifer = nc.aquifer();
            // walk the interpolation loop to the target cell
            int cw = ns.getCellWidth(), ch = ns.getCellHeight(), minY = ns.minY();
            int cellCountX = 16 / cw, cellCountZ = 16 / cw, cellCountY = ns.height() / ch;
            int lx = bx - cx * 16, lz = bz - cz * 16;
            double density = Double.NaN;
            nc.initializeForFirstCellX();
            for (int ccx = 0; ccx < cellCountX && Double.isNaN(density); ccx++) {
                nc.advanceCellX(ccx);
                for (int ccz = 0; ccz < cellCountZ && Double.isNaN(density); ccz++) {
                    for (int cy = cellCountY - 1; cy >= 0 && Double.isNaN(density); cy--) {
                        nc.selectCellYZ(cy, ccz);
                        for (int yic = ch - 1; yic >= 0 && Double.isNaN(density); yic--) {
                            int posY = (minY / ch + cy) * ch + yic;
                            nc.updateForY(posY, (double) yic / ch);
                            for (int zic = ch - 1; zic >= 0 && Double.isNaN(density); zic--) {
                                for (int xic = 0; xic < cw && Double.isNaN(density); xic++) {
                                    int px = ccx * cw + xic;
                                    int py = (minY / ch + cy) * ch + yic;
                                    int pz = ccz * cw + zic;
                                    if (px == lx && py == by && pz == lz) {
                                        density = nc.getInterpolatedDensity();
                                        var st = aquifer.computeSubstance(
                                            new net.minecraft.world.level.levelgen.DensityFunction.SinglePointContext(bx, by, bz),
                                            density);
                                        String name = st == null ? "NULL(default)"
                                            : st.getBlock().getName().getString();
                                        System.out.println("SUBSTANCE " + bx + " " + by + " " + bz
                                            + " density=" + String.format(java.util.Locale.ROOT, "%.6f", density)
                                            + " -> " + name);
                                    }
                                }
                            }
                        }
                    }
                }
            }
            if (Double.isNaN(density)) {
                System.out.println("SUBSTANCE " + bx + " " + by + " " + bz + " NOT-FOUND");
            }
        }
    }

}
