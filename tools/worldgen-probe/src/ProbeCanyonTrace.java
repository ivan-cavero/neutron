import java.util.Locale;
import net.minecraft.util.Mth;
import net.minecraft.util.RandomSource;
import net.minecraft.world.level.levelgen.LegacyRandomSource;
import net.minecraft.world.level.levelgen.WorldgenRandom;

/** Trace the full CANYON carver pipeline for one target chunk.
 *  Replicates CanyonWorldCarver.isStartChunk / carve / doCarve /
 *  initWidthFactors / updateVerticalRadius / shouldSkip-index math and
 *  WorldCarver.canReach + carveEllipsoid range logic verbatim from the
 *  26.2 decompile, using the REAL WorldgenRandom(LegacyRandomSource) and
 *  RandomSource.createThreadLocalInstance(SingleThreadedRandomSource).
 *  All floats/doubles are emitted as raw-bit hex for mechanical diffing.
 *
 *  args: [seed] [targetX] [targetZ]   (defaults: 424242 7 2)
 */
public class ProbeCanyonTrace {
    static final int RANGE_BLOCKS = (4 * 2 - 1) * 16; // getRange()=4 -> 112
    static final int GEN_DEPTH = 384;
    static final int MIN_GEN_Y = -64;

    static String h(float v) { return String.format(Locale.ROOT, "%08x", Float.floatToRawIntBits(v)); }
    static String hd(double v) { return String.format(Locale.ROOT, "%016x", Double.doubleToRawLongBits(v)); }

    public static void main(String[] args) throws Exception {
        long seed = args.length > 0 ? Long.parseLong(args[0]) : 424242L;
        int tcx = args.length > 1 ? Integer.parseInt(args[1]) : 7;
        int tcz = args.length > 2 ? Integer.parseInt(args[2]) : 2;
        System.out.printf(Locale.ROOT, "PROBE=java seed=%d target=(%d,%d)%n", seed, tcx, tcz);
        // Vanilla order: dx OUTER (-8..8), dz INNER (NoiseBasedChunkGenerator.java:326-327)
        int fired = 0;
        for (int dx = -8; dx <= 8; dx++) {
            for (int dz = -8; dz <= 8; dz++) {
                if (traceSource(seed, tcx + dx, tcz + dz, tcx, tcz)) fired++;
            }
        }
        System.out.printf(Locale.ROOT, "FIRED %d%n", fired);
    }

    static boolean traceSource(long levelSeed, int scx, int scz, int tcx, int tcz) {
        WorldgenRandom rng = new WorldgenRandom(new LegacyRandomSource(0L));
        rng.setLargeFeatureSeed(levelSeed + 2, scx, scz); // canyon index=2 within AIR carver list
        float f = rng.nextFloat();
        if (!(f <= 0.01F)) return false; // CanyonWorldCarver.isStartChunk (:20-22)
        System.out.printf(Locale.ROOT, "START src=(%d,%d) f=%s%n", scx, scz, h(f));
        carve(rng, scx, scz, tcx, tcz);
        return true;
    }

    // --- CanyonWorldCarver.carve (:24-63) ---
    static void carve(WorldgenRandom rng, int scx, int scz, int tcx, int tcz) {
        int maxDistance = RANGE_BLOCKS;
        double x = scx * 16 + rng.nextInt(16);                 // :35
        int y = Mth.randomBetweenInclusive(rng, 10, 67);       // :36 UniformHeight absolute [10..67]
        double z = scz * 16 + rng.nextInt(16);                 // :37
        float horizontalRotation = rng.nextFloat() * (float) (Math.PI * 2); // :38
        float verticalRotation = Mth.randomBetween(rng, -0.125F, 0.125F);   // :39
        double yScale = 3.0F;                                  // :40 ConstantFloat
        float thickness = trapezoid(rng, 0.0F, 6.0F, 2.0F);    // :41 TrapezoidFloat(0,6,plateau=2)
        int distance = (int) (maxDistance * Mth.randomBetween(rng, 0.75F, 1.0F)); // :42 shape.distanceFactor
        long tunnelSeed = rng.nextLong();                      // :49
        System.out.printf(Locale.ROOT,
            "P src=(%d,%d) x=%s y=%s z=%s yaw=%s pitch=%s th=%s dist=%d tseed=%016x%n",
            scx, scz, hd(x), hd(y), hd(z), h(horizontalRotation), h(verticalRotation),
            h(thickness), distance, tunnelSeed);
        doCarve(tunnelSeed, x, y, z, thickness, horizontalRotation, verticalRotation,
                0, distance, yScale, tcx, tcz, scx, scz);
    }

    static float trapezoid(WorldgenRandom r, float min, float max, float plateau) { // TrapezoidFloat.java:35-40
        float range = max - min;
        float plateauStart = (range - plateau) / 2.0F;
        float plateauEnd = range - plateauStart;
        return min + r.nextFloat() * plateauEnd + r.nextFloat() * plateauStart;
    }

    // --- CanyonWorldCarver.doCarve (:65-126) ---
    static void doCarve(long tunnelSeed, double x, double y, double z, final float thickness,
                        float horizontalRotation, float verticalRotation,
                        final int step, final int distance, final double yScale,
                        int tcx, int tcz, int scx, int scz) {
        RandomSource random = RandomSource.createThreadLocalInstance(tunnelSeed); // :83
        float[] widthFactorPerHeight = initWidthFactors(random, scx, scz);        // :84
        float yRota = 0.0F;
        float xRota = 0.0F;
        int reachSteps = 0, firstHitStep = -1, lastHitStep = -1, skipDrawSteps = 0;

        for (int currentStep = step; currentStep < distance; currentStep++) {     // :88
            // :89 NOTE: currentStep*(float)Math.PI is FLOAT math, then /distance as FLOAT div, f2d into Mth.sin(D)F
            double horizontalRadius = 1.5 + Mth.sin(currentStep * (float) Math.PI / distance) * thickness;
            double verticalRadius = horizontalRadius * yScale;                    // :90
            horizontalRadius *= Mth.randomBetween(random, 0.75F, 1.0F);           // :91 hrf uniform [0.75,1)
            verticalRadius = updateVerticalRadius(random, verticalRadius, distance, currentStep); // :92
            float xc = Mth.cos(verticalRotation);                                 // :93
            float xs = Mth.sin(verticalRotation);                                 // :94
            x += Mth.cos(horizontalRotation) * xc;                                // :95
            y += xs;                                                              // :96
            z += Mth.sin(horizontalRotation) * xc;                                // :97
            verticalRotation *= 0.7F;                                             // :98
            verticalRotation += xRota * 0.05F;                                    // :99
            horizontalRotation += yRota * 0.05F;                                  // :100
            xRota *= 0.8F;                                                        // :101
            yRota *= 0.5F;                                                        // :102
            xRota += (random.nextFloat() - random.nextFloat()) * random.nextFloat() * 2.0F; // :103
            yRota += (random.nextFloat() - random.nextFloat()) * random.nextFloat() * 4.0F; // :104
            if (random.nextInt(4) != 0) {                                         // :105 skip-draw
                boolean reach = canReach(tcx, tcz, x, z, currentStep, distance, thickness); // :106
                if (!reach) {
                    logStep(scx, scz, currentStep, x, y, z, horizontalRadius, verticalRadius, true, false, false, tcx, tcz);
                    System.out.printf(Locale.ROOT,
                        "ABORT src=(%d,%d) at_step=%d reach_steps=%d first_hit_step=%d last_hit_step=%d skipdraw_steps=%d%n",
                        scx, scz, currentStep, reachSteps, firstHitStep, lastHitStep, skipDrawSteps);
                    return;                                                       // :107
                }
                StepRange r = ellipsoidRanges(x, y, z, horizontalRadius, verticalRadius, tcx, tcz);
                if (r.writesLocal()) {                                            // touches target chunk with a non-empty Y loop
                    if (firstHitStep < 0) firstHitStep = currentStep;
                    lastHitStep = currentStep;
                }
                logStep(scx, scz, currentStep, x, y, z, horizontalRadius, verticalRadius, true, true, !r.earlyOut, tcx, tcz);
                reachSteps++;
            } else {
                skipDrawSteps++;
                logStep(scx, scz, currentStep, x, y, z, horizontalRadius, verticalRadius, false, false, false, tcx, tcz);
            }
        }
        System.out.printf(Locale.ROOT,
            "END src=(%d,%d) steps_run=%d reach_steps=%d first_hit_step=%d last_hit_step=%d skipdraw_steps=%d%n",
            scx, scz, distance - step, reachSteps, firstHitStep, lastHitStep, skipDrawSteps);
    }

    /** i-th width factor record lines + summary. initWidthFactors (:128-142), widthSmoothness=3. */
    static float[] initWidthFactors(RandomSource random, int scx, int scz) {
        int depth = GEN_DEPTH;                        // context.getGenDepth()
        float[] widthFactorPerHeight = new float[depth];
        float widthFactor = 1.0F;
        int renew = 0;
        for (int yIndex = 0; yIndex < depth; yIndex++) {
            if (yIndex == 0 || random.nextInt(3) == 0) {
                widthFactor = 1.0F + random.nextFloat() * random.nextFloat();
                renew++;
            }
            widthFactorPerHeight[yIndex] = widthFactor * widthFactor;
        }
        // emit band-relevant factors: band y in [33..64] -> shouldSkip index = y-MIN_GEN_Y-1 in [96..127]
        for (int i = 96; i <= 127; i++) {
            System.out.printf(Locale.ROOT, "W src=(%d,%d) idx=%d wf=%s%n", scx, scz, i, h(widthFactorPerHeight[i]));
        }
        int wsum = 0;
        for (int i = 0; i < depth; i++) wsum += Float.floatToRawIntBits(widthFactorPerHeight[i]);
        System.out.printf(Locale.ROOT, "WSUM src=(%d,%d) sum=%08x nrenew=%d%n", scx, scz, wsum, renew);
        return widthFactorPerHeight;
    }

    // --- updateVerticalRadius (:144-150): vRadiusDefault=1.0, vRadiusCenter=0.0 ---
    static double updateVerticalRadius(RandomSource random, double verticalRadius,
                                       float distance, float currentStep) {
        float verticalMultiplier = 1.0F - Math.abs(0.5F - currentStep / distance) * 2.0F;
        float factor = 1.0F + 0.0F * verticalMultiplier;
        return factor * verticalRadius * Mth.randomBetween(random, 0.75F, 1.0F);
    }

    // --- WorldCarver.canReach (:208-218) ---
    static boolean canReach(int tcx, int tcz, double x, double z, int currentStep, int totalSteps, float thickness) {
        double xMid = tcx * 16 + 8.0;
        double zMid = tcz * 16 + 8.0;
        double xd = x - xMid;
        double zd = z - zMid;
        double remaining = totalSteps - currentStep;
        double rr = thickness + 2.0F + 16.0F;
        return xd * xd + zd * zd - remaining * remaining <= rr * rr;
    }

    static final class StepRange {
        final boolean earlyOut; final int minXIndex, maxXIndex, minY, maxY, minZIndex, maxZIndex;
        StepRange(boolean e, int a,int b,int c,int d,int e2,int f){earlyOut=e;minXIndex=a;maxXIndex=b;minY=c;maxY=d;minZIndex=e2;maxZIndex=f;}
        boolean lxValid(){ return minXIndex <= maxXIndex && minZIndex <= maxZIndex; }
        boolean writesLocal(){ return lxValid() && maxY > minY; }
        boolean overlapsBand(int lo, int hi){ return !(maxY < lo || minY > hi); }
    }

    /** Mirror of WorldCarver.carveEllipsoid range computation (:76-113) without block access. */
    static StepRange ellipsoidRanges(double x, double y, double z, double hr, double vr, int tcx, int tcz) {
        double centerX = tcx * 16 + 8.0, centerZ = tcz * 16 + 8.0;   // getMiddleBlockX/Z
        double maxDelta = 16.0 + hr * 2.0;
        if (Math.abs(x - centerX) > maxDelta || Math.abs(z - centerZ) > maxDelta) {
            return new StepRange(true, 0,0,0,0,0,0);
        }
        int chunkMinX = tcx * 16, chunkMinZ = tcz * 16;
        int minXIndex = Math.max(Mth.floor(x - hr) - chunkMinX - 1, 0);
        int maxXIndex = Math.min(Mth.floor(x + hr) - chunkMinX, 15);
        int minY = Math.max(Mth.floor(y - vr) - 1, MIN_GEN_Y + 1);
        int maxY = Math.min(Mth.floor(y + vr) + 1, MIN_GEN_Y + GEN_DEPTH - 1 - 7);
        int minZIndex = Math.max(Mth.floor(z - hr) - chunkMinZ - 1, 0);
        int maxZIndex = Math.min(Mth.floor(z + hr) - chunkMinZ, 15);
        return new StepRange(false, minXIndex, maxXIndex, minY, maxY, minZIndex, maxZIndex);
    }

    static void logStep(int scx, int scz, int i, double x, double y, double z,
                        double hr, double vr, boolean drawn, boolean reachPass,
                        boolean rangeComputed, int tcx, int tcz) {
        String lx="-,-", lz="-,-", ly="-,-";
        if (drawn && rangeComputed) {
            StepRange r = ellipsoidRanges(x, y, z, hr, vr, tcx, tcz);
            lx = r.minXIndex + "," + r.maxXIndex;
            lz = r.minZIndex + "," + r.maxZIndex;
            ly = r.minY + "," + r.maxY;
        }
        System.out.printf(Locale.ROOT,
            "S src=(%d,%d) i=%d x=%s y=%s z=%s yaw=PITCHHELD hr=%s vr=%s draw=%d reach=%d rngc=%d lx=[%s] lz=[%s] ly=[%s]%n",
            scx, scz, i, hd(x), hd(y), hd(z), hd(hr), hd(vr),
            drawn ? 1 : 0, reachPass ? 1 : 0, rangeComputed ? 1 : 0, lx, lz, ly);
    }
}
