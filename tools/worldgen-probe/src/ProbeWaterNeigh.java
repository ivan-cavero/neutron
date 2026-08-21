import java.util.ArrayList;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Locale;
import java.util.Set;
import net.minecraft.SharedConstants;
import net.minecraft.core.Holder;
import net.minecraft.core.HolderGetter;
import net.minecraft.core.registries.Registries;
import net.minecraft.data.registries.VanillaRegistries;
import net.minecraft.server.Bootstrap;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.levelgen.Aquifer;
import net.minecraft.world.level.levelgen.DensityFunctions;
import net.minecraft.world.level.levelgen.NoiseBasedChunkGenerator;
import net.minecraft.world.level.levelgen.NoiseChunk;
import net.minecraft.world.level.levelgen.NoiseGeneratorSettings;
import net.minecraft.world.level.levelgen.NoiseSettings;
import net.minecraft.world.level.levelgen.RandomState;
import net.minecraft.world.level.levelgen.blending.Blender;
import net.minecraft.world.level.levelgen.synth.NormalNoise;

/**
 * Seed 424242: vanilla doFill (BeardifierMarker) at ref water cells AND their
 * 6 neighbours. SpringFeature needs exactly 1 air among NESW+below.
 *
 *   java -cp "bin:$CP" ProbeWaterNeigh 424242
 */
public class ProbeWaterNeigh {
    static class NC extends NoiseChunk {
        NC(int cellXZ, RandomState rs, int x, int z, NoiseSettings ns,
           DensityFunctions.BeardifierOrMarker beard, NoiseGeneratorSettings set,
           Aquifer.FluidPicker fluid, Blender b) {
            super(cellXZ, rs, x, z, ns, beard, set, fluid, b);
        }
        double interp() { return this.getInterpolatedDensity(); }
        BlockState state() { return this.getInterpolatedState(); }
    }

    static final int[][] WATER = {
        {12,1,15},{10,2,15},{8,3,14},{2,5,14},{5,5,14},{1,5,15},{8,3,15},{2,5,15},{5,5,15},
        {1,6,21},{3,6,23},
        {0,5,17},{1,5,17},{0,5,18},{1,6,19},{0,6,20},{2,6,22},{0,5,24},{1,5,25},
        {1,4,28},{2,4,28},{3,4,28}
    };
    static final int[][] DIR = {
        {0,1,0},{0,-1,0},{-1,0,0},{1,0,0},{0,0,-1},{0,0,1}
    };
    static final String[] DIRN = {"above","below","west","east","north","south"};

    public static void main(String[] args) throws Exception {
        long seed = args.length > 0 ? Long.parseLong(args[0]) : 424242L;
        SharedConstants.tryDetectVersion();
        Bootstrap.bootStrap();
        var lookup = VanillaRegistries.createLookup();
        HolderGetter<NormalNoise.NoiseParameters> noises = lookup.lookupOrThrow(Registries.NOISE);
        Holder<NoiseGeneratorSettings> settings =
            lookup.lookupOrThrow(Registries.NOISE_SETTINGS).getOrThrow(NoiseGeneratorSettings.OVERWORLD);
        RandomState rs = RandomState.create(settings.value(), noises, seed);
        var gen = new NoiseBasedChunkGenerator(
            new net.minecraft.world.level.biome.FixedBiomeSource(
                lookup.lookupOrThrow(Registries.BIOME).getOrThrow(net.minecraft.world.level.biome.Biomes.PLAINS)),
            settings);
        var fp = NoiseBasedChunkGenerator.class.getDeclaredField("globalFluidPicker");
        fp.setAccessible(true);
        @SuppressWarnings("unchecked")
        var fluid = (Aquifer.FluidPicker) ((java.util.function.Supplier<?>) fp.get(gen)).get();
        var beardCls = Class.forName("net.minecraft.world.level.levelgen.DensityFunctions$BeardifierMarker");
        var bf = beardCls.getField("INSTANCE");
        bf.setAccessible(true);
        @SuppressWarnings("unchecked")
        var beard = (DensityFunctions.BeardifierOrMarker) bf.get(null);

        System.out.println("limitation=BeardifierMarker seed=" + seed);
        Set<String> want = new LinkedHashSet<>();
        List<int[]> pts = new ArrayList<>();
        for (int[] w : WATER) {
            add(want, pts, w[0], w[1], w[2]);
            for (int[] d : DIR) add(want, pts, w[0] + d[0], w[1] + d[1], w[2] + d[2]);
        }

        // group by chunk
        java.util.Map<String, List<int[]>> byChunk = new java.util.TreeMap<>();
        for (int[] p : pts) {
            int cx = Math.floorDiv(p[0], 16), cz = Math.floorDiv(p[2], 16);
            byChunk.computeIfAbsent(cx + "," + cz, k -> new ArrayList<>()).add(p);
        }
        java.util.Map<String, Sample> got = new java.util.HashMap<>();
        for (var e : byChunk.entrySet()) {
            String[] sp = e.getKey().split(",");
            int cx = Integer.parseInt(sp[0]), cz = Integer.parseInt(sp[1]);
            sampleChunk(rs, settings, beard, fluid, cx, cz, e.getValue(), got);
        }

        int springShaped = 0;
        for (int[] w : WATER) {
            Sample c = got.get(key(w[0], w[1], w[2]));
            System.out.printf(Locale.ROOT, "CELL %d %d %d density=%+.6f %s block=%s%n",
                w[0], w[1], w[2], c.density, c.density > 0 ? "solid" : "open", blockName(c.state));
            int holes = 0, rocks = 0;
            for (int i = 0; i < DIR.length; i++) {
                int nx = w[0] + DIR[i][0], ny = w[1] + DIR[i][1], nz = w[2] + DIR[i][2];
                Sample n = got.get(key(nx, ny, nz));
                boolean open = n.density <= 0;
                if (open) holes++;
                else rocks++;
                System.out.printf(Locale.ROOT, "  %s (%d,%d,%d) density=%+.6f %s block=%s%n",
                    DIRN[i], nx, ny, nz, n.density, open ? "open" : "solid", blockName(n.state));
            }
            // Spring counts NESW+below only (not above). above must be valid/solid.
            Sample above = got.get(key(w[0], w[1] + 1, w[2]));
            int holeNB = 0, rockNB = 0;
            int[][] nb = {{-1,0,0},{1,0,0},{0,0,-1},{0,0,1},{0,-1,0}};
            for (int[] d : nb) {
                Sample n = got.get(key(w[0] + d[0], w[1] + d[1], w[2] + d[2]));
                if (n.density <= 0) holeNB++; else rockNB++;
            }
            boolean aboveSolid = above.density > 0;
            boolean shaped = aboveSolid && rockNB == 4 && holeNB == 1 && c.density > 0;
            if (shaped) springShaped++;
            System.out.printf(Locale.ROOT,
                "  spring_shape above_solid=%s nesw+below rock=%d hole=%d SHAPED=%s%n",
                aboveSolid, rockNB, holeNB, shaped);
        }
        System.out.println("FINDING spring_shaped_solid_cells=" + springShaped + "/" + WATER.length);
        System.out.println("cite NoiseChunk.getInterpolatedDensity / getInterpolatedState BeardifierMarker; SpringFeature.place rockCount=4 holeCount=1");
    }

    static void add(Set<String> want, List<int[]> pts, int x, int y, int z) {
        String k = key(x, y, z);
        if (want.add(k)) pts.add(new int[] {x, y, z});
    }
    static String key(int x, int y, int z) { return x + "," + y + "," + z; }
    static String blockName(BlockState st) {
        if (st == null) return "null";
        return String.valueOf(net.minecraft.core.registries.BuiltInRegistries.BLOCK.getKey(st.getBlock()));
    }

    static class Sample { double density; BlockState state; }

    static void sampleChunk(RandomState rs, Holder<NoiseGeneratorSettings> settings,
                            DensityFunctions.BeardifierOrMarker beard, Aquifer.FluidPicker fluid,
                            int chunkX, int chunkZ, List<int[]> pts, java.util.Map<String, Sample> out) {
        NoiseSettings ns = settings.value().noiseSettings();
        int cw = ns.getCellWidth(), ch = ns.getCellHeight(), minY = ns.minY();
        int cellCountX = 16 / cw, cellCountZ = 16 / cw, cellCountY = ns.height() / ch;
        java.util.Set<String> need = new java.util.HashSet<>();
        for (int[] p : pts) need.add(key(p[0], p[1], p[2]));
        var nc = new NC(4, rs, chunkX * 16, chunkZ * 16, ns, beard, settings.value(), fluid, Blender.empty());
        nc.initializeForFirstCellX();
        int left = need.size();
        for (int cx = 0; cx < cellCountX && left > 0; cx++) {
            nc.advanceCellX(cx);
            for (int cz = 0; cz < cellCountZ && left > 0; cz++) {
                for (int cy = cellCountY - 1; cy >= 0 && left > 0; cy--) {
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
                                String k = key(posX, posY, posZ);
                                if (need.contains(k) && !out.containsKey(k)) {
                                    Sample s = new Sample();
                                    s.density = nc.interp();
                                    s.state = nc.state();
                                    out.put(k, s);
                                    left--;
                                }
                            }
                        }
                    }
                }
            }
            nc.swapSlices();
        }
    }
}
