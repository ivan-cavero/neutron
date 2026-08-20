import java.util.Locale;
import net.minecraft.SharedConstants;
import net.minecraft.core.Holder;
import net.minecraft.core.HolderGetter;
import net.minecraft.core.registries.Registries;
import net.minecraft.data.registries.VanillaRegistries;
import net.minecraft.server.Bootstrap;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.levelgen.Aquifer;
import net.minecraft.world.level.levelgen.DensityFunctions;
import net.minecraft.world.level.levelgen.LegacyRandomSource;
import net.minecraft.world.level.levelgen.NoiseBasedChunkGenerator;
import net.minecraft.world.level.levelgen.NoiseChunk;
import net.minecraft.world.level.levelgen.NoiseGeneratorSettings;
import net.minecraft.world.level.levelgen.NoiseSettings;
import net.minecraft.world.level.levelgen.RandomState;
import net.minecraft.world.level.levelgen.WorldgenRandom;
import net.minecraft.world.level.levelgen.blending.Blender;
import net.minecraft.world.level.levelgen.synth.NormalNoise;

/** isStartChunk counts + aquifer.computeSubstance(ctx, 0.0) at water cells.
 *  carve_visit is unknown without a full ProtoChunk carve. */
public class ProbeCarveHits {
    static class NC extends NoiseChunk {
        NC(int cellXZ, RandomState rs, int x, int z, NoiseSettings ns,
           DensityFunctions.BeardifierOrMarker beard, NoiseGeneratorSettings set,
           Aquifer.FluidPicker fluid, Blender b) {
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

        System.out.println("carve_visit=unknown (no ProtoChunk carve); density0 = getCarveState path");
        for (int[] t : new int[][] {{0, 0}, {0, 1}}) {
            starts(seed, t[0], t[1]);
        }

        int[][] pts = {
            {12,1,15},{10,2,15},{8,3,14},{2,5,14},{5,5,14},{1,5,15},{8,3,15},{2,5,15},{5,5,15},
            {1,6,21},{3,6,23}
        };
        NoiseSettings ns = settings.value().noiseSettings();
        for (int[] p : pts) {
            int bx = p[0], by = p[1], bz = p[2];
            int cx = Math.floorDiv(bx, 16), cz = Math.floorDiv(bz, 16);
            var nc = new NC(4, rs, cx * 16, cz * 16, ns, beard, settings.value(), fluid, Blender.empty());
            // Position the chunk as FunctionContext via one cell select + updates.
            int cw = ns.getCellWidth(), ch = ns.getCellHeight(), minY = ns.minY();
            int lx = Math.floorMod(bx, 16), lz = Math.floorMod(bz, 16);
            int cellX = lx / cw, cellZ = lz / cw, cellY = (by - minY) / ch;
            int xic = lx % cw, yic = Math.floorMod(by - minY, ch), zic = lz % cw;
            nc.initializeForFirstCellX();
            for (int i = 0; i <= cellX; i++) nc.advanceCellX(i);
            nc.selectCellYZ(cellY, cellZ);
            nc.updateForY(by, (double) yic / ch);
            nc.updateForX(bx, (double) xic / cw);
            nc.updateForZ(bz, (double) zic / cw);
            BlockState st = nc.aquifer().computeSubstance(nc, 0.0);
            String name = st == null ? "null" : String.valueOf(
                net.minecraft.core.registries.BuiltInRegistries.BLOCK.getKey(st.getBlock()));
            System.out.printf(Locale.ROOT, "%d %d %d carve_visit=unknown density0_block=%s%n",
                bx, by, bz, name);
        }
    }

    static void starts(long seed, int tcx, int tcz) {
        int starts = 0;
        System.out.printf(Locale.ROOT, "target=(%d,%d)%n", tcx, tcz);
        for (int dz = -8; dz <= 8; dz++) {
            for (int dx = -8; dx <= 8; dx++) {
                int scx = tcx + dx, scz = tcz + dz;
                for (int index = 0; index < 3; index++) {
                    WorldgenRandom rng = new WorldgenRandom(new LegacyRandomSource(0L));
                    rng.setLargeFeatureSeed(seed + index, scx, scz);
                    float f = rng.nextFloat();
                    float p = index == 0 ? 0.15f : index == 1 ? 0.07f : 0.01f;
                    if (f <= p) {
                        starts++;
                        if (index < 2) {
                            int a = rng.nextInt(15) + 1;
                            int b = rng.nextInt(a) + 1;
                            int caveCount = rng.nextInt(b);
                            // first cave y (CaveWorldCarver): uniform y_min..y_max after more rolls
                            System.out.printf(Locale.ROOT,
                                "  START source=(%d,%d) idx=%d f=%.4f caveCount=%d%n",
                                scx, scz, index, f, caveCount);
                        } else {
                            System.out.printf(Locale.ROOT, "  START source=(%d,%d) idx=2 canyon f=%.4f%n",
                                scx, scz, f);
                        }
                    }
                }
            }
        }
        System.out.printf(Locale.ROOT, "  total_starts=%d%n", starts);
    }
}
