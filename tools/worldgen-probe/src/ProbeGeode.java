import net.minecraft.util.Mth;
import net.minecraft.world.level.levelgen.WorldgenRandom;
import net.minecraft.world.level.levelgen.XoroshiroRandomSource;
import net.minecraft.world.level.levelgen.LegacyRandomSource;
import net.minecraft.world.level.levelgen.synth.NormalNoise;
import net.minecraft.util.valueproviders.UniformInt;

/**
 * AgentE geode dump probe (26.2, seed 424242).
 *
 * Replicates the vanilla pre-GeodeFeature RNG chain with the REAL vanilla
 * classes:
 *   1. decorationSeed = WorldgenRandom(Xoroshiro(seed)).setDecorationSeed(seed, ox0, oz0)
 *   2. setFeatureSeed(dec, GLOBAL_IDX=2, STEP=2)   // amethyst_geode, step LOCAL_MODIFICATIONS
 *   3. RarityFilter:  nextFloat() < 1/24
 *   4. InSquare:      nextInt(16) x, then z
 *   5. UniformHeight: Mth.randomBetweenInclusive(random, -58, 30)
 *   6. GeodeFeature.place head: distributionPoints(3..4), crackSize draw,
 *      crack-chance draw, per-point outerWallDistance(4..6)*3 + pointOffset(1..2).
 *
 * Also prints the geode noise construction draws (LegacyRandomSource LCG):
 *   NormalNoise.create(WorldgenRandom(LegacyRandomSource(levelSeed)), -4, 1.0)
 * consumes exactly one forkPositional nextLong per PerlinNoise.
 *
 * Usage: java ProbeGeode <seed> <cx> <cz> [<cx> <cz>...]
 */
public class ProbeGeode {
    static final int GEODE_GLOBAL_INDEX = 2; // FeatureSorter step-2 slot (26.2 overworld)
    static final int STEP = 2;               // GenerationStep.Decoration.LOCAL_MODIFICATIONS

    public static void main(String[] args) {
        long seed = Long.parseLong(args[0]);

        // --- noise-construction cross-check (level-seed LCG) ---
        WorldgenRandom noiseRng = new WorldgenRandom(new LegacyRandomSource(seed));
        NormalNoise noise = NormalNoise.create(noiseRng, -4, 1.0);
        System.out.println("noise: valueFactor=" + 0.16666666666666666 / (0.1 * (1.0 + 1.0 / 1)));
        System.out.println("hash(octave_-4)=" + "octave_-4".hashCode());

        for (int i = 1; i < args.length; i += 2) {
            int cx = Integer.parseInt(args[i]);
            int cz = Integer.parseInt(args[i + 1]);
            traceOrigin(seed, cx, cz);
        }
    }

    static void traceOrigin(long seed, int cx, int cz) {
        int ox = cx * 16, oz = cz * 16;
        WorldgenRandom rng = new WorldgenRandom(new XoroshiroRandomSource(seed));
        long dec = rng.setDecorationSeed(seed, ox, oz);
        rng.setFeatureSeed(dec, GEODE_GLOBAL_INDEX, STEP);

        float rarityRoll = rng.nextFloat();
        boolean fire = rarityRoll < 1.0f / 24.0f;
        StringBuilder sb = new StringBuilder();
        sb.append(String.format("chunk (%d,%d) dec=%d roll=%.6f fire=%b", cx, cz, dec, rarityRoll, fire));
        if (!fire) {
            System.out.println(sb);
            return;
        }
        int px = ox + rng.nextInt(16);
        int pz = oz + rng.nextInt(16);
        int py = Mth.randomBetweenInclusive(rng, -58, 30);

        // GeodeFeature.place head (config defaults of amethyst_geode.json):
        // distribution_points UniformInt(3,4); outer_wall_distance UniformInt(4,6);
        // point_offset UniformInt(1,2); invalid_blocks_threshold=1.
        UniformInt distPoints = UniformInt.of(3, 4);
        UniformInt wallDist = UniformInt.of(4, 6);
        UniformInt ptOffset = UniformInt.of(1, 2);

        int numPoints = distPoints.sample(rng);
        double crackSizeAdjustment = (double) numPoints / 6.0;
        double crackSize = 1.0 / Math.sqrt(2.0 + rng.nextDouble() / 2.0 + (numPoints > 3 ? crackSizeAdjustment : 0.0));
        boolean shouldCrack = rng.nextFloat() < 0.95;
        sb.append(String.format(" origin=(%d,%d,%d) numPoints=%d crackSize=%.4f shouldCrack=%b", px, py, pz, numPoints, crackSize, shouldCrack));

        int[][] points = new int[numPoints][3];
        int[] offs = new int[numPoints];
        for (int p = 0; p < numPoints; p++) {
            int dx = wallDist.sample(rng), dy = wallDist.sample(rng), dz = wallDist.sample(rng);
            points[p][0] = px + dx; points[p][1] = py + dy; points[p][2] = pz + dz;
            offs[p] = ptOffset.sample(rng);
        }
        sb.append(" points=");
        for (int p = 0; p < numPoints; p++) {
            sb.append(String.format("(%d,%d,%d)+%d ", points[p][0], points[p][1], points[p][2], offs[p]));
        }
        if (shouldCrack) {
            int oi = rng.nextInt(4);
            int co = numPoints * 2 + 1;
            int cdx = oi == 0 || oi == 2 ? co : 0;
            int cdz = oi == 1 || oi == 2 ? co : 0;
            sb.append(String.format(" crackOffsetIdx=%d crackBase=(%d,+{7,5,1},%d)", oi, px + cdx, pz + cdz));
        }
        System.out.println(sb);
    }
}
