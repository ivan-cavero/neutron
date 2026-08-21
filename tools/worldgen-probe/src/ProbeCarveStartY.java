import java.util.ArrayList;
import java.util.List;
import java.util.Locale;
import java.util.TreeMap;
import net.minecraft.SharedConstants;
import net.minecraft.core.Holder;
import net.minecraft.core.HolderGetter;
import net.minecraft.core.registries.Registries;
import net.minecraft.data.registries.VanillaRegistries;
import net.minecraft.resources.Identifier;
import net.minecraft.resources.ResourceKey;
import net.minecraft.server.Bootstrap;
import net.minecraft.util.Mth;
import net.minecraft.util.RandomSource;
import net.minecraft.world.level.ChunkPos;
import net.minecraft.world.level.LevelHeightAccessor;
import net.minecraft.world.level.biome.Biomes;
import net.minecraft.world.level.biome.FixedBiomeSource;
import net.minecraft.world.level.levelgen.LegacyRandomSource;
import net.minecraft.world.level.levelgen.NoiseBasedChunkGenerator;
import net.minecraft.world.level.levelgen.NoiseGeneratorSettings;
import net.minecraft.world.level.levelgen.RandomState;
import net.minecraft.world.level.levelgen.WorldGenerationContext;
import net.minecraft.world.level.levelgen.WorldgenRandom;
import net.minecraft.world.level.levelgen.carver.CanyonCarverConfiguration;
import net.minecraft.world.level.levelgen.carver.CaveCarverConfiguration;
import net.minecraft.world.level.levelgen.carver.ConfiguredWorldCarver;
import net.minecraft.world.level.levelgen.synth.NormalNoise;

/**
 * Seed 424242: vanilla CaveWorldCarver / CanyonWorldCarver start (x,y,z)
 * via real HeightProvider.sample, plus createTunnel / doCanyon motion-only
 * (vanilla Mth.sin/cos) for sources that can reach targets (0,0) and (0,1).
 *
 * CaveWorldCarver.carve / UniformHeight.sample / WorldGenerationContext
 */
public class ProbeCarveStartY {
    static final long SEED = 424242L;
    static final int APPLY_RANGE = 8;
    static final int RANGE_BLOCKS = 7 * 16; // getRange()=4 → sectionToBlockCoord(7)

    static class Hit {
        String kind;
        int scx, scz, x, y, z;
        int caveCount;
        int inst;
        int band00, band01, in00, in01, write00, write01;
        int ymin, ymax, steps;
        boolean anyTunnel;
    }

    public static void main(String[] args) throws Exception {
        long seed = args.length > 0 ? Long.parseLong(args[0]) : SEED;
        SharedConstants.tryDetectVersion();
        Bootstrap.bootStrap();
        var lookup = VanillaRegistries.createLookup();
        HolderGetter<NormalNoise.NoiseParameters> noises = lookup.lookupOrThrow(Registries.NOISE);
        Holder<NoiseGeneratorSettings> settings =
            lookup.lookupOrThrow(Registries.NOISE_SETTINGS).getOrThrow(NoiseGeneratorSettings.OVERWORLD);
        RandomState rs = RandomState.create(settings.value(), noises, seed);
        var gen = new NoiseBasedChunkGenerator(
            new FixedBiomeSource(lookup.lookupOrThrow(Registries.BIOME).getOrThrow(Biomes.PLAINS)),
            settings);
        WorldGenerationContext wgc = new WorldGenerationContext(gen, LevelHeightAccessor.create(-64, 384));
        System.out.printf(Locale.ROOT, "minY=%d height=%d (WorldGenerationContext)%n",
            wgc.getMinGenY(), wgc.getGenDepth());

        var carvers = lookup.lookupOrThrow(Registries.CONFIGURED_CARVER);
        ConfiguredWorldCarver<?> cave = carvers.getOrThrow(
            ResourceKey.create(Registries.CONFIGURED_CARVER, Identifier.parse("minecraft:cave"))).value();
        ConfiguredWorldCarver<?> extra = carvers.getOrThrow(
            ResourceKey.create(Registries.CONFIGURED_CARVER, Identifier.parse("minecraft:cave_extra_underground"))).value();
        ConfiguredWorldCarver<?> canyon = carvers.getOrThrow(
            ResourceKey.create(Registries.CONFIGURED_CARVER, Identifier.parse("minecraft:canyon"))).value();
        CaveCarverConfiguration caveCfg = (CaveCarverConfiguration) cave.config();
        CaveCarverConfiguration extraCfg = (CaveCarverConfiguration) extra.config();
        CanyonCarverConfiguration canyonCfg = (CanyonCarverConfiguration) canyon.config();
        System.out.printf(Locale.ROOT, "cave p=%.4f extra p=%.4f canyon p=%.4f%n",
            caveCfg.probability, extraCfg.probability, canyonCfg.probability);

        List<Hit> hits = new ArrayList<>();
        int[] startHits = new int[3];
        int[] caveCountZero = new int[2];

        // Union of sources for targets (0,0) and (0,1): cx=-8..8, cz=-8..9
        for (int scx = -8; scx <= 8; scx++) {
            for (int scz = -8; scz <= 9; scz++) {
                dumpCaves("cave", 0, caveCfg, wgc, seed, scx, scz, hits, startHits, caveCountZero);
                dumpCaves("cave_extra", 1, extraCfg, wgc, seed, scx, scz, hits, startHits, caveCountZero);
                dumpCanyon(canyonCfg, wgc, seed, scx, scz, hits, startHits);
            }
        }

        TreeMap<Integer, Integer> bins = new TreeMap<>();
        int n32 = 0;
        int band00 = 0, band01 = 0, write00 = 0, write01 = 0;
        int wormsBand00 = 0, wormsBand01 = 0;
        for (Hit h : hits) {
            bins.merge((h.y / 16) * 16, 1, Integer::sum);
            if (h.y >= -32 && h.y < 0) n32++;
            band00 += h.band00;
            band01 += h.band01;
            write00 += h.write00;
            write01 += h.write01;
            if (h.band00 > 0) wormsBand00++;
            if (h.band01 > 0) wormsBand01++;
        }

        System.out.println();
        System.out.printf(Locale.ROOT,
            "isStartChunk hits: cave=%d cave_extra=%d canyon=%d (total=%d)%n",
            startHits[0], startHits[1], startHits[2],
            startHits[0] + startHits[1] + startHits[2]);
        System.out.printf(Locale.ROOT, "isStart with cave_count=0: cave=%d cave_extra=%d%n",
            caveCountZero[0], caveCountZero[1]);
        System.out.printf(Locale.ROOT, "sampled start instances (caves with Y + canyons): %d%n", hits.size());
        System.out.println("start Y 16-high bins:");
        for (var e : bins.entrySet()) {
            System.out.printf(Locale.ROOT, "  y[%d,%d) n=%d%n", e.getKey(), e.getKey() + 16, e.getValue());
        }
        int minY = hits.stream().mapToInt(h -> h.y).min().orElse(0);
        int maxY = hits.stream().mapToInt(h -> h.y).max().orElse(0);
        System.out.printf(Locale.ROOT, "start Y min=%d max=%d%n", minY, maxY);
        System.out.printf(Locale.ROOT, "starts with Y in [-32,0): %d%n", n32);
        for (Hit h : hits) {
            if (h.y >= -32 && h.y < 0) {
                System.out.printf(Locale.ROOT, "    %s source=(%d,%d) pos=(%d,%d,%d)%n",
                    h.kind, h.scx, h.scz, h.x, h.y, h.z);
            }
        }

        System.out.println();
        System.out.println("=== worm motion (createTunnel / doCanyon, vanilla Mth) ===");
        for (Hit h : hits) {
            if (!h.anyTunnel) continue;
            if (h.band00 + h.band01 + h.in00 + h.in01 == 0) continue;
            System.out.printf(Locale.ROOT,
                "WORM %s source=(%d,%d) pos=(%d,%d,%d) steps=%d ymin=%d ymax=%d in00=%d in01=%d band00=%d band01=%d write00=%d write01=%d%n",
                h.kind, h.scx, h.scz, h.x, h.y, h.z, h.steps, h.ymin, h.ymax,
                h.in00, h.in01, h.band00, h.band01, h.write00, h.write01);
        }
        System.out.printf(Locale.ROOT,
            "SUMMARY worms_with_band00=%d worms_with_band01=%d total_band00_steps=%d total_band01_steps=%d write00=%d write01=%d%n",
            wormsBand00, wormsBand01, band00, band01, write00, write01);

        int[][] water = {
            {12, 1, 15}, {10, 2, 15}, {8, 3, 14}, {2, 5, 14}, {5, 5, 14},
            {1, 5, 15}, {8, 3, 15}, {2, 5, 15}, {5, 5, 15}, {1, 6, 21}, {3, 6, 23}
        };
        System.out.println("closest start (euclidean) to water cells:");
        for (int[] w : water) {
            double best = Double.POSITIVE_INFINITY;
            Hit bh = null;
            for (Hit h : hits) {
                double d = dist(h.x + 0.5, h.y + 0.5, h.z + 0.5, w[0] + 0.5, w[1] + 0.5, w[2] + 0.5);
                if (d < best) { best = d; bh = h; }
            }
            if (bh != null) {
                System.out.printf(Locale.ROOT,
                    "  (%d,%d,%d) dist=%.2f start=%s source=(%d,%d) pos=(%d,%d,%d)%n",
                    w[0], w[1], w[2], best, bh.kind, bh.scx, bh.scz, bh.x, bh.y, bh.z);
            }
        }
    }

    static double dist(double x, double y, double z, double a, double b, double c) {
        double dx = x - a, dy = y - b, dz = z - c;
        return Math.sqrt(dx * dx + dy * dy + dz * dz);
    }

    static void dumpCaves(
        String kind, int index, CaveCarverConfiguration cfg, WorldGenerationContext wgc,
        long seed, int scx, int scz, List<Hit> hits, int[] startHits, int[] caveCountZero
    ) {
        WorldgenRandom rng = new WorldgenRandom(new LegacyRandomSource(0L));
        rng.setLargeFeatureSeed(seed + index, scx, scz);
        if (rng.nextFloat() > cfg.probability) return;
        startHits[index]++;
        int a = rng.nextInt(15) + 1;
        int b = rng.nextInt(a) + 1;
        int caveCount = rng.nextInt(b);
        if (caveCount == 0) {
            caveCountZero[index]++;
            System.out.printf(Locale.ROOT, "START %s source=(%d,%d) cave_count=0%n", kind, scx, scz);
            return;
        }
        ChunkPos cp = new ChunkPos(scx, scz);
        for (int i = 0; i < caveCount; i++) {
            double x = cp.getBlockX(rng.nextInt(16));
            int y = cfg.y.sample(rng, wgc);
            double z = cp.getBlockZ(rng.nextInt(16));
            System.out.printf(Locale.ROOT,
                "START %s source=(%d,%d) pos=(%d,%d,%d) caveCount=%d i=%d%n",
                kind, scx, scz, (int) x, y, (int) z, caveCount, i);

            double horizMult = cfg.horizontalRadiusMultiplier.sample(rng);
            double vertMult = cfg.verticalRadiusMultiplier.sample(rng);
            cfg.floorLevel.sample(rng); // consume
            int tunnelCount = 1;
            if (rng.nextInt(4) == 0) {
                cfg.yScale.sample(rng);
                rng.nextFloat(); // room thickness 1+nextFloat*6
                tunnelCount += rng.nextInt(4);
            }
            Hit hit = new Hit();
            hit.kind = kind;
            hit.scx = scx;
            hit.scz = scz;
            hit.x = (int) x;
            hit.y = y;
            hit.z = (int) z;
            hit.caveCount = caveCount;
            hit.inst = i;
            hit.ymin = Integer.MAX_VALUE;
            hit.ymax = Integer.MIN_VALUE;
            for (int t = 0; t < tunnelCount; t++) {
                float yaw = rng.nextFloat() * 6.2831855f;
                float pitch = (rng.nextFloat() - 0.5f) / 4.0f;
                float thickness = getThickness(rng);
                int branchCount = RANGE_BLOCKS - rng.nextInt(RANGE_BLOCKS / 4);
                long tseed = rng.nextLong();
                walkTunnel(hit, tseed, x, y, z, horizMult, vertMult, thickness, yaw, pitch, 0, branchCount);
            }
            hits.add(hit);
        }
    }

    static void dumpCanyon(
        CanyonCarverConfiguration cfg, WorldGenerationContext wgc,
        long seed, int scx, int scz, List<Hit> hits, int[] startHits
    ) {
        WorldgenRandom rng = new WorldgenRandom(new LegacyRandomSource(0L));
        rng.setLargeFeatureSeed(seed + 2, scx, scz);
        if (rng.nextFloat() > cfg.probability) return;
        startHits[2]++;
        ChunkPos cp = new ChunkPos(scx, scz);
        double x = cp.getBlockX(rng.nextInt(16));
        int y = cfg.y.sample(rng, wgc);
        double z = cp.getBlockZ(rng.nextInt(16));
        System.out.printf(Locale.ROOT, "START canyon source=(%d,%d) pos=(%d,%d,%d)%n",
            scx, scz, (int) x, y, (int) z);
        float yaw = rng.nextFloat() * 6.2831855f;
        float pitch = cfg.verticalRotation.sample(rng);
        cfg.yScale.sample(rng);
        float thickness = cfg.shape.thickness.sample(rng);
        int branchCount = (int) (RANGE_BLOCKS * cfg.shape.distanceFactor.sample(rng));
        long tseed = rng.nextLong();
        Hit hit = new Hit();
        hit.kind = "canyon";
        hit.scx = scx;
        hit.scz = scz;
        hit.x = (int) x;
        hit.y = y;
        hit.z = (int) z;
        hit.ymin = Integer.MAX_VALUE;
        hit.ymax = Integer.MIN_VALUE;
        walkCanyon(hit, tseed, x, y, z, thickness, yaw, pitch, 0, branchCount);
        hits.add(hit);
    }

    static float getThickness(RandomSource rng) {
        float t = rng.nextFloat() * 2.0f + rng.nextFloat();
        if (rng.nextInt(10) == 0) {
            t *= rng.nextFloat() * rng.nextFloat() * 3.0f + 1.0f;
        }
        return t;
    }

    static void walkTunnel(
        Hit hit, long seed, double x, double y, double z,
        double horizMult, double vertMult, float thickness,
        float yaw, float pitch, int branchIndex, int branchCount
    ) {
        RandomSource rng = RandomSource.createThreadLocalInstance(seed);
        int steeperAt = rng.nextInt(branchCount / 2) + branchCount / 4;
        boolean rare = rng.nextInt(6) == 0;
        float yawVel = 0, pitchVel = 0;
        for (int i = branchIndex; i < branchCount; i++) {
            float angle = 3.1415927f * i / (float) branchCount;
            float sinV = Mth.sin((double) angle);
            double horizBase = 1.5 + (double) (sinV * thickness);
            double horiz = horizBase * horizMult;
            double vert = horizBase * 1.0 * vertMult;
            float cosPitch = Mth.cos((double) pitch);
            x += (double) (Mth.cos((double) yaw) * cosPitch);
            y += (double) Mth.sin((double) pitch);
            z += (double) (Mth.sin((double) yaw) * cosPitch);
            pitch *= rare ? 0.92f : 0.7f;
            pitch += pitchVel * 0.1f;
            yaw += yawVel * 0.1f;
            pitchVel *= 0.9f;
            yawVel *= 0.75f;
            pitchVel += (rng.nextFloat() - rng.nextFloat()) * rng.nextFloat() * 2.0f;
            yawVel += (rng.nextFloat() - rng.nextFloat()) * rng.nextFloat() * 4.0f;
            if (i == steeperAt && thickness > 1.0f) {
                walkTunnel(hit, rng.nextLong(), x, y, z, horizMult, vertMult,
                    rng.nextFloat() * 0.5f + 0.5f, yaw - 1.5707964f, pitch / 3.0f, i, branchCount);
                walkTunnel(hit, rng.nextLong(), x, y, z, horizMult, vertMult,
                    rng.nextFloat() * 0.5f + 0.5f, yaw + 1.5707964f, pitch / 3.0f, i, branchCount);
                return;
            }
            if (rng.nextInt(4) == 0) continue;
            note(hit, x, y, z, horiz, vert);
        }
    }

    static void walkCanyon(
        Hit hit, long seed, double x, double y, double z,
        float thickness, float yaw, float pitch, int branchIndex, int branchCount
    ) {
        RandomSource rng = RandomSource.createThreadLocalInstance(seed);
        // consume widthFactors RNG (genDepth=384, widthSmoothness=3)
        float w = 1.0f;
        for (int yy = 0; yy < 384; yy++) {
            if (yy == 0 || rng.nextInt(3) == 0) {
                w = 1.0f + rng.nextFloat() * rng.nextFloat();
            }
        }
        float yawVel = 0, pitchVel = 0;
        for (int i = branchIndex; i < branchCount; i++) {
            float sinV = Mth.sin((double) (3.1415927f * i / (float) branchCount));
            double horiz = (1.5 + (double) (sinV * thickness)) * (0.75 + rng.nextFloat() * 0.25);
            // updateVerticalRadius consumes one nextFloat (randomBetween 0.75..1.0)
            rng.nextFloat();
            double vert = horiz * 3.0; // yScale constant 3, then * factor; size only for note()
            float cosPitch = Mth.cos((double) pitch);
            x += (double) (Mth.cos((double) yaw) * cosPitch);
            y += (double) Mth.sin((double) pitch);
            z += (double) (Mth.sin((double) yaw) * cosPitch);
            pitch *= 0.7f;
            pitch += pitchVel * 0.05f;
            yaw += yawVel * 0.05f;
            pitchVel *= 0.8f;
            yawVel *= 0.5f;
            pitchVel += (rng.nextFloat() - rng.nextFloat()) * rng.nextFloat() * 2.0f;
            yawVel += (rng.nextFloat() - rng.nextFloat()) * rng.nextFloat() * 4.0f;
            if (rng.nextInt(4) == 0) continue;
            note(hit, x, y, z, horiz, vert);
        }
    }

    static void note(Hit hit, double x, double y, double z, double horiz, double vert) {
        hit.anyTunnel = true;
        hit.steps++;
        int iy = (int) Math.floor(y);
        if (iy < hit.ymin) hit.ymin = iy;
        if (iy > hit.ymax) hit.ymax = iy;
        int cx = (int) Math.floor(x) >> 4;
        int cz = (int) Math.floor(z) >> 4;
        // ellipsoid can cover extra blocks; treat step as entering if center in chunk
        // OR ellipsoid range overlaps the chunk (horiz reach)
        boolean overlap00 = overlapsChunk(x, z, horiz, 0, 0);
        boolean overlap01 = overlapsChunk(x, z, horiz, 0, 1);
        boolean yBand = y > -16 - vert && y < 16 + vert; // ellipsoid Y overlap with [-16,16)
        if (cx == 0 && cz == 0) hit.in00++;
        if (cx == 0 && cz == 1) hit.in01++;
        if (overlap00 && yBand) hit.band00++;
        if (overlap01 && yBand) hit.band01++;
        if (overlap00 && yBand && localXzNonEmpty(x, z, horiz, 0, 0)) hit.write00++;
        if (overlap01 && yBand && localXzNonEmpty(x, z, horiz, 0, 1)) hit.write01++;
    }

    static boolean overlapsChunk(double x, double z, double horiz, int tcx, int tcz) {
        double midX = tcx * 16 + 8;
        double midZ = tcz * 16 + 8;
        double reach = 16.0 + horiz * 2.0;
        return Math.abs(x - midX) <= reach && Math.abs(z - midZ) <= reach;
    }

    /** WorldCarver.carveEllipsoid local x/z range in the target chunk. */
    static boolean localXzNonEmpty(double x, double z, double horiz, int tcx, int tcz) {
        int minBx = tcx * 16, minBz = tcz * 16;
        int minLx = Math.max(Mth.floor(x - horiz) - minBx - 1, 0);
        int maxLx = Math.min(Mth.floor(x + horiz) - minBx, 15);
        int minLz = Math.max(Mth.floor(z - horiz) - minBz - 1, 0);
        int maxLz = Math.min(Mth.floor(z + horiz) - minBz, 15);
        return minLx <= maxLx && minLz <= maxLz;
    }
}
