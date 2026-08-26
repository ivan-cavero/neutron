import java.util.Locale;
import net.minecraft.SharedConstants;
import net.minecraft.core.Holder;
import net.minecraft.core.HolderGetter;
import net.minecraft.core.BlockPos;
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
import net.minecraft.world.level.levelgen.Aquifer;

/** Vanilla aquifer computeSubstance at explicit world cells.
 *  Usage: ProbeAquiferSubstance seed cx cz [x y z]...  (cx,cz = chunk holding the cell) */
public class ProbeAquiferSubstance {
    static class NC extends NoiseChunk {
        NC(int cellXZ, RandomState rs, int x, int z, NoiseSettings ns,
           DensityFunctions.BeardifierOrMarker beard, NoiseGeneratorSettings set,
           Aquifer.FluidPicker fluid, Blender b) {
            super(cellXZ, rs, x, z, ns, beard, set, fluid, b);
        }
    }
    public static void main(String[] args) throws Exception {
        long seed = Long.parseLong(args[0]);
        int cx = Integer.parseInt(args[1]);
        int cz = Integer.parseInt(args[2]);
        SharedConstants.tryDetectVersion();
        Bootstrap.bootStrap();
        var lookup = VanillaRegistries.createLookup();
        HolderGetter<NormalNoise.NoiseParameters> noises = lookup.lookupOrThrow(Registries.NOISE);
        Holder<NoiseGeneratorSettings> settings =
            lookup.lookupOrThrow(Registries.NOISE_SETTINGS).getOrThrow(NoiseGeneratorSettings.OVERWORLD);
        RandomState rs = RandomState.create(settings.value(), noises, seed);
        var gen = new net.minecraft.world.level.levelgen.NoiseBasedChunkGenerator(
            new net.minecraft.world.level.biome.FixedBiomeSource(
                lookup.lookupOrThrow(Registries.BIOME).getOrThrow(net.minecraft.world.level.biome.Biomes.LUSH_CAVES)),
            settings);
        var f = net.minecraft.world.level.levelgen.NoiseBasedChunkGenerator.class.getDeclaredField("globalFluidPicker");
        f.setAccessible(true);
        @SuppressWarnings("unchecked")
        var fluid = (Aquifer.FluidPicker)((java.util.function.Supplier<?>) f.get(gen)).get();
        var beardCls = Class.forName("net.minecraft.world.level.levelgen.DensityFunctions$BeardifierMarker");
        var bf = beardCls.getField("INSTANCE"); bf.setAccessible(true);
        @SuppressWarnings("unchecked")
        var beard = (DensityFunctions.BeardifierOrMarker) bf.get(null);
        var ns = settings.value().noiseSettings();
        NC nc = new NC(8, rs, cx * 16, cz * 16, ns, beard, settings.value(), fluid, Blender.empty());
        Aquifer aq = Aquifer.create(nc, new net.minecraft.world.level.ChunkPos(cx, cz), rs.router(),
            rs.aquiferRandom(), ns.minY(), ns.height(), fluid);
        for (int i = 3; i + 2 < args.length; i += 3) {
            int x = Integer.parseInt(args[i]), y = Integer.parseInt(args[i+1]), z = Integer.parseInt(args[i+2]);
            BlockState s = aq.computeSubstance(
                new net.minecraft.world.level.levelgen.DensityFunction.SinglePointContext(x, y, z), 0.0);
            String name = s == null ? "null" : s.getBlock().toString();
            System.out.println("(" + x + "," + y + "," + z + ") -> " + name);
        }
    }
}