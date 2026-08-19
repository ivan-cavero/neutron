import net.minecraft.SharedConstants;
import net.minecraft.core.Holder;
import net.minecraft.core.HolderGetter;
import net.minecraft.core.registries.Registries;
import net.minecraft.data.registries.VanillaRegistries;
import net.minecraft.resources.Identifier;
import net.minecraft.resources.ResourceKey;
import net.minecraft.server.Bootstrap;
import net.minecraft.world.level.biome.Biome;
import net.minecraft.world.level.biome.Climate;
import net.minecraft.world.level.biome.BiomeManager;
import net.minecraft.world.level.biome.MultiNoiseBiomeSource;
import net.minecraft.util.LinearCongruentialGenerator;
import net.minecraft.world.level.levelgen.NoiseGeneratorSettings;
import net.minecraft.world.level.levelgen.RandomState;
import net.minecraft.world.level.levelgen.synth.NormalNoise;

/** Grid dump: vanilla biome + climate target (depth etc.) at cave-level cells.
 *  Prints one line per (x,z) column for y in {0,16,32,48,64,72,80,88,96,104}:
 *  x z y depth temp humid cont ero weir biome
 *  Columns chosen to cover the 3x3 around (0,0) incl. the mismatched cells. */
public class ProbeCaveGrid {
    public static void main(String[] args) throws Exception {
        long seed = Long.parseLong(args[0]);
        SharedConstants.tryDetectVersion();
        Bootstrap.bootStrap();
        var lookup = VanillaRegistries.createLookup();
        HolderGetter<NormalNoise.NoiseParameters> noises = lookup.lookupOrThrow(Registries.NOISE);
        var settings = lookup.lookupOrThrow(Registries.NOISE_SETTINGS).getOrThrow(NoiseGeneratorSettings.OVERWORLD);
        RandomState rs = RandomState.create(settings.value(), noises, seed);
        var registry = lookup.lookupOrThrow(Registries.MULTI_NOISE_BIOME_SOURCE_PARAMETER_LIST);
        var key = ResourceKey.create(Registries.MULTI_NOISE_BIOME_SOURCE_PARAMETER_LIST, Identifier.parse("minecraft:overworld"));
        var source = MultiNoiseBiomeSource.createFromPreset(registry.getOrThrow(key));
        var sampler = rs.sampler();
        int[][] cols = {
            {-16,-16}, {-16,-8}, {-16,0}, {-16,8}, {-16,16},
            {-8,-16}, {-8,-8}, {-8,0}, {-8,8}, {-8,16},
            {0,-16}, {0,-8}, {0,0}, {0,8}, {0,16},
            {8,-16}, {8,-8}, {8,0}, {8,8}, {8,16},
            {16,-16}, {16,-8}, {16,0}, {16,8}, {16,16},
        };
        int[] ys = {0, 16, 32, 48, 64, 72, 80, 88, 96, 104};
        long zoom = BiomeManager.obfuscateSeed(seed);
        for (int[] c : cols) {
            int x = c[0], z = c[1];
            for (int y : ys) {
                // full vanilla classification path: voronoi quart + target at it
                int absX = x - 2, absY = y - 2, absZ = z - 2;
                int parentX = absX >> 2, parentY = absY >> 2, parentZ = absZ >> 2;
                double fractX = (absX & 3) / 4.0, fractY = (absY & 3) / 4.0, fractZ = (absZ & 3) / 4.0;
                int minI = 0;
                double minF = Double.POSITIVE_INFINITY;
                for (int i = 0; i < 8; i++) {
                    boolean xEven = (i & 4) == 0, yEven = (i & 2) == 0, zEven = (i & 1) == 0;
                    int cX = xEven ? parentX : parentX + 1;
                    int cY = yEven ? parentY : parentY + 1;
                    int cZ = zEven ? parentZ : parentZ + 1;
                    double dX = xEven ? fractX : fractX - 1.0;
                    double dY = yEven ? fractY : fractY - 1.0;
                    double dZ = zEven ? fractZ : fractZ - 1.0;
                    double f = getFiddledDistance(zoom, cX, cY, cZ, dX, dY, dZ);
                    if (minF > f) { minI = i; minF = f; }
                }
                int bX = (minI & 4) == 0 ? parentX : parentX + 1;
                int bY = (minI & 2) == 0 ? parentY : parentY + 1;
                int bZ = (minI & 1) == 0 ? parentZ : parentZ + 1;
                Climate.TargetPoint t = sampler.sample(bX, bY, bZ);
                Holder<Biome> b = source.getNoiseBiome(bX, bY, bZ, sampler);
                String bn = b.unwrapKey().map(k -> k.identifier().toString().replace("minecraft:", "")).orElse("?");
                System.out.println(x + " " + z + " " + y
                    + " q=" + bX + "," + bY + "," + bZ
                    + " d=" + t.depth()
                    + " t=" + t.temperature()
                    + " h=" + t.humidity()
                    + " c=" + t.continentalness()
                    + " e=" + t.erosion()
                    + " w=" + t.weirdness()
                    + " biome=" + bn);
            }
        }
    }
    static double getFiddledDistance(long seed, int x, int y, int z, double dX, double dY, double dZ) {
        long r = seed;
        long[] arr = {x, y, z, x, y, z};
        for (long a : arr) r = LinearCongruentialGenerator.next(r, a);
        double fX = getFiddle(r);
        r = LinearCongruentialGenerator.next(r, seed);
        double fY = getFiddle(r);
        r = LinearCongruentialGenerator.next(r, seed);
        double fZ = getFiddle(r);
        return (dX + fX) * (dX + fX) + (dY + fY) * (dY + fY) + (dZ + fZ) * (dZ + fZ);
    }
    static double getFiddle(long rval) {
        double uniform = Math.floorMod(rval >> 24, 1024) / 1024.0;
        return (uniform - 0.5) * 0.9;
    }
}
