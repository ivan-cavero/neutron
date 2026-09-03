import net.minecraft.SharedConstants;
import net.minecraft.server.Bootstrap;
import net.minecraft.core.Holder;
import net.minecraft.core.HolderGetter;
import net.minecraft.core.registries.Registries;
import net.minecraft.data.registries.VanillaRegistries;
import net.minecraft.world.level.levelgen.*;
import net.minecraft.world.level.levelgen.blending.Blender;
import net.minecraft.world.level.levelgen.synth.NormalNoise;
import net.minecraft.world.level.biome.FixedBiomeSource;
import net.minecraft.world.level.biome.Biomes;
import net.minecraft.world.level.block.Blocks;
import net.minecraft.world.level.block.state.BlockState;
// Replicates the REAL frozenOceanExtension math for one column using the real
// NoiseChunk.preliminarySurfaceLevel and real noises. Args: seed cx cz [cx cz ...]
// Prints msl, berg gate, top/bottom bands per column.
public class ProbeIcebergMsl {
    static class NC extends NoiseChunk {
        NC(int cellXZ, RandomState rs, int x, int z, NoiseSettings ns,
           DensityFunctions.BeardifierOrMarker beard, NoiseGeneratorSettings set,
           Aquifer.FluidPicker fluid, Blender b) {
            super(cellXZ, rs, x, z, ns, beard, set, fluid, b);
        }
        int psl(int bx, int bz) { return this.preliminarySurfaceLevel(bx, bz); }
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
        NoiseSettings ns = settings.value().noiseSettings();
        var fpField = NoiseBasedChunkGenerator.class.getDeclaredField("globalFluidPicker");
        fpField.setAccessible(true);
        var gen = new NoiseBasedChunkGenerator(
            new FixedBiomeSource(lookup.lookupOrThrow(Registries.BIOME).getOrThrow(Biomes.PLAINS)), settings);
        var fluid = (Aquifer.FluidPicker) ((java.util.function.Supplier<?>) fpField.get(gen)).get();
        var beardCls = Class.forName("net.minecraft.world.level.levelgen.DensityFunctions$BeardifierMarker");
        var f = beardCls.getField("INSTANCE"); f.setAccessible(true);
        var beard = (DensityFunctions.BeardifierOrMarker) f.get(null);

        NormalNoise surface = rs.getOrCreateNoise(Noises.ICEBERG_SURFACE);
        NormalNoise pillar = rs.getOrCreateNoise(Noises.ICEBERG_PILLAR);
        NormalNoise roof = rs.getOrCreateNoise(Noises.ICEBERG_PILLAR_ROOF);
        int sea = settings.value().seaLevel();

        for (int i = 1; i + 1 < args.length; i += 2) {
            int x = Integer.parseInt(args[i]);
            int z = Integer.parseInt(args[i + 1]);
            // NoiseChunk per 16-block cell containing this column
            int cellX = x >> 4, cellZ = z >> 4;
            NC nc = new NC(4, rs, cellX * 16, cellZ * 16, ns, beard, settings.value(), fluid, Blender.empty());
            // msl: bilinear over the 2x2 cell-corner preliminary surface levels
            int c0 = nc.psl((x & ~15), (z & ~15));
            int c1 = nc.psl((x & ~15) + 16, (z & ~15));
            int c2 = nc.psl((x & ~15), (z & ~15) + 16);
            int c3 = nc.psl((x & ~15) + 16, (z & ~15) + 16);
            double lerp = net.minecraft.util.Mth.lerp2(
                (x & 15) / 16.0F, (z & 15) / 16.0F, c0, c1, c2, c3);
            // surfaceDepth: (int)(surfaceNoise*2.75 + 3 + rand*0.25)
            var rndField = RandomState.class.getDeclaredField("random");
            rndField.setAccessible(true);
            var depthRand = ((net.minecraft.world.level.levelgen.PositionalRandomFactory) rndField.get(rs)).at(x, 0, z);
            double sd = surface.getValue(x, 0.0, z) * 2.75 + 3.0 + depthRand.nextDouble() * 0.25;
            int msl = net.minecraft.util.Mth.floor(lerp) + (int) sd - 8;
            double s = Math.abs(surface.getValue(x, 0.0, z) * 8.25);
            double p = Math.abs(pillar.getValue(x * 1.28, 0.0, z * 1.28) * 15.0);
            double berg = Math.min(s, p);
            double r = Math.abs(roof.getValue(x * 1.17, 0.0, z * 1.17) * 1.5);
            double top = Math.min(berg * berg * 1.2, Math.ceil(r * 40.0) + 14.0);
            boolean fills = berg > 1.8 && top > 2.0;
            double extTop = fills ? top + sea : 0.0;
            double extBot = fills ? sea - top - 7.0 : 0.0;
            System.out.printf("x=%d z=%d msl=%d berg=%.4f topRaw=%.3f fires=%s extTop=%.2f extBot=%.2f band=[%d..%d]%n",
                x, z, msl, berg, top, berg > 1.8, extTop, extBot,
                fills ? Math.max(64, (int) extTop + 1) : -1, fills ? msl : -1);
        }
    }
}
