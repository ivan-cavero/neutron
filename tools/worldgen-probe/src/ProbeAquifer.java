import net.minecraft.core.Holder;
import net.minecraft.core.HolderGetter;
import net.minecraft.core.QuartPos;
import net.minecraft.core.registries.Registries;
import net.minecraft.data.registries.VanillaRegistries;
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
}
