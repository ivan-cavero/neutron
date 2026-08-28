import java.lang.reflect.Field;
import java.util.ArrayList;
import java.util.List;
import java.util.Locale;
import java.util.function.Function;
import net.minecraft.SharedConstants;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Holder;
import net.minecraft.core.HolderGetter;
import net.minecraft.core.HolderLookup;
import net.minecraft.core.registries.Registries;
import net.minecraft.data.registries.VanillaRegistries;
import net.minecraft.server.Bootstrap;
import net.minecraft.world.level.ChunkPos;
import net.minecraft.world.level.LevelHeightAccessor;
import net.minecraft.world.level.biome.Biome;
import net.minecraft.world.level.biome.Biomes;
import net.minecraft.world.level.biome.FixedBiomeSource;
import net.minecraft.world.level.block.Blocks;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.chunk.CarvingMask;
import net.minecraft.world.level.chunk.ChunkAccess;
import net.minecraft.world.level.chunk.LevelChunkSection;
import net.minecraft.world.level.chunk.ProtoChunk;
import net.minecraft.world.ticks.ProtoChunkTicks;
import net.minecraft.world.level.chunk.UpgradeData;
import net.minecraft.world.level.levelgen.Aquifer;
import net.minecraft.world.level.levelgen.LegacyRandomSource;
import net.minecraft.world.level.levelgen.NoiseBasedChunkGenerator;
import net.minecraft.world.level.levelgen.NoiseChunk;
import net.minecraft.world.level.levelgen.NoiseGeneratorSettings;
import net.minecraft.world.level.levelgen.NoiseSettings;
import net.minecraft.world.level.levelgen.RandomState;
import net.minecraft.world.level.levelgen.WorldgenRandom;
import net.minecraft.world.level.levelgen.blending.Blender;
import net.minecraft.world.level.levelgen.carver.CaveCarverConfiguration;
import net.minecraft.world.level.levelgen.carver.CaveWorldCarver;
import net.minecraft.world.level.levelgen.carver.CarvingContext;
import net.minecraft.world.level.levelgen.carver.WorldCarver;
import net.minecraft.world.level.levelgen.DensityFunctions;
import net.minecraft.world.level.levelgen.synth.NormalNoise;

/**
 * Traces every cave-carver carveEllipsoid call for ONE target chunk,
 * replicating NoiseBasedChunkGenerator.applyCarvers (dx outer, dz inner,
 * setLargeFeatureSeed(seed + carverIndex, source)) with the REAL
 * CaveWorldCarver over a stone-filled ProtoChunk + real NoiseChunk aquifer.
 *
 * Output: "SRC <sx> <sz> <index>" per started source, then per ellipsoid
 * "EL <x> <y> <z> <hR> <vR>" (%.6f, same format as the Rust trace).
 *
 * Usage: ProbeCarveTrace <seed> <targetCx> <targetCz>
 */
public class ProbeCarveTrace {
    static final int MINY = -64, TOP = 320, HEIGHT = TOP - MINY;

    static class TracingCaveCarver extends CaveWorldCarver {
        TracingCaveCarver() {
            super(CaveCarverConfiguration.CODEC);
        }

        @Override
        protected boolean carveEllipsoid(
                CarvingContext context,
                CaveCarverConfiguration configuration,
                ChunkAccess chunk,
                Function<BlockPos, Holder<Biome>> biomeGetter,
                Aquifer aquifer,
                double x, double y, double z,
                double horizontalRadius, double verticalRadius,
                CarvingMask mask,
                WorldCarver.CarveSkipChecker skipChecker) {
            System.out.printf(Locale.ROOT, "EL %.6f %.6f %.6f %.6f %.6f%n",
                    x, y, z, horizontalRadius, verticalRadius);
            return super.carveEllipsoid(context, configuration, chunk, biomeGetter,
                    aquifer, x, y, z, horizontalRadius, verticalRadius, mask, skipChecker);
        }
    }

    public static void main(String[] args) throws Exception {
        long seed = Long.parseLong(args[0]);
        int tcx = Integer.parseInt(args[1]);
        int tcz = Integer.parseInt(args[2]);
        SharedConstants.tryDetectVersion();
        Bootstrap.bootStrap();
        HolderLookup.Provider lookup = VanillaRegistries.createLookup();
        HolderGetter<NormalNoise.NoiseParameters> noises = lookup.lookupOrThrow(Registries.NOISE);
        Holder<NoiseGeneratorSettings> settings =
                lookup.lookupOrThrow(Registries.NOISE_SETTINGS).getOrThrow(NoiseGeneratorSettings.OVERWORLD);
        RandomState rs = RandomState.create(settings.value(), noises, seed);
        var biomeReg = lookup.lookupOrThrow(Registries.BIOME);
        Holder<Biome> plains = biomeReg.getOrThrow(Biomes.PLAINS);
        var gen = new NoiseBasedChunkGenerator(new FixedBiomeSource(plains), settings);
        var factory = ProbeDecorate.palettedContainerFactoryManual(lookup);

        LevelHeightAccessor lha = new LevelHeightAccessor() {
            @Override public int getMinY() { return MINY; }
            @Override public int getHeight() { return HEIGHT; }
            @Override public int getMaxY() { return TOP; }
            @Override public boolean isOutsideBuildHeight(int y) { return y < MINY || y >= TOP; }
            @Override public int getMinSectionY() { return MINY >> 4; }
            @Override public int getMaxSectionY() { return (TOP >> 4) - 1; }
            @Override public int getSectionsCount() { return HEIGHT / 16; }
            @Override public int getSectionIndex(int y) { return (y >> 4) - (MINY >> 4); }
            @Override public int getSectionIndexFromSectionY(int sy) { return sy - (MINY >> 4); }
        };

        LevelChunkSection[] secs = new LevelChunkSection[HEIGHT / 16];
        BlockState stone = Blocks.STONE.defaultBlockState();
        for (int s = 0; s < secs.length; s++) {
            var statesC = factory.createForBlockStates();
            var biomesC = factory.createForBiomes();
            for (int ly = 0; ly < 16; ly++)
                for (int lz = 0; lz < 16; lz++)
                    for (int lx = 0; lx < 16; lx++)
                        statesC.set(lx, ly, lz, stone);
            for (int sy = 0; sy < 4; sy++)
                for (int bz4 = 0; bz4 < 4; bz4++)
                    for (int bx4 = 0; bx4 < 4; bx4++)
                        biomesC.set(bx4, sy, bz4, plains);
            secs[s] = new LevelChunkSection(statesC, biomesC);
        }
        ProtoChunk chunk = new ProtoChunk(new ChunkPos(tcx, tcz), UpgradeData.EMPTY, secs,
                new ProtoChunkTicks<>(), new ProtoChunkTicks<>(), lha, factory, null);

        NoiseSettings ns = settings.value().noiseSettings().clampToHeightAccessor(lha);
        var beardCls = Class.forName("net.minecraft.world.level.levelgen.DensityFunctions$BeardifierMarker");
        var bf = beardCls.getField("INSTANCE");
        bf.setAccessible(true);
        var beard = (DensityFunctions.BeardifierOrMarker) bf.get(null);
        Field fpField = NoiseBasedChunkGenerator.class.getDeclaredField("globalFluidPicker");
        fpField.setAccessible(true);
        var fluid = (Aquifer.FluidPicker) ((java.util.function.Supplier<?>) fpField.get(gen)).get();
        NoiseChunk noiseChunk = new NoiseChunk(4, rs, tcx * 16, tcz * 16, ns, beard,
                settings.value(), fluid, Blender.empty()) {};
        Aquifer aquifer = noiseChunk.aquifer();

        net.minecraft.core.RegistryAccess regAccess =
                (net.minecraft.core.RegistryAccess) ProbeDecorate.regAccessStub(lookup);
        CarvingContext context = new CarvingContext(
                gen, regAccess, lha, noiseChunk, rs, settings.value().surfaceRule());
        CarvingMask mask = new CarvingMask(HEIGHT, MINY);

        var carverReg = lookup.lookupOrThrow(Registries.CONFIGURED_CARVER);
        CaveCarverConfiguration caveCfg = (CaveCarverConfiguration)
                carverReg.getOrThrow(net.minecraft.data.worldgen.Carvers.CAVE).value().config();
        CaveCarverConfiguration caveExtraCfg = (CaveCarverConfiguration)
                carverReg.getOrThrow(net.minecraft.data.worldgen.Carvers.CAVE_EXTRA_UNDERGROUND).value().config();

        CaveWorldCarver tracer = new TracingCaveCarver();
        Function<BlockPos, Holder<Biome>> biomeGetter = p -> plains;

        for (int dx = -8; dx <= 8; dx++) {
            for (int dz = -8; dz <= 8; dz++) {
                int scx = tcx + dx, scz = tcz + dz;
                WorldgenRandom random = new WorldgenRandom(new LegacyRandomSource(0L));

                random.setLargeFeatureSeed(seed + 0, scx, scz);
                if (tracer.isStartChunk(caveCfg, random)) {
                    System.out.printf(Locale.ROOT, "SRC %d %d 0%n", scx, scz);
                    tracer.carve(context, caveCfg, chunk, biomeGetter, random, aquifer,
                            new ChunkPos(scx, scz), mask);
                }

                random.setLargeFeatureSeed(seed + 1, scx, scz);
                if (tracer.isStartChunk(caveExtraCfg, random)) {
                    System.out.printf(Locale.ROOT, "SRC %d %d 1%n", scx, scz);
                    tracer.carve(context, caveExtraCfg, chunk, biomeGetter, random, aquifer,
                            new ChunkPos(scx, scz), mask);
                }
            }
        }
        System.out.flush();
    }
}
