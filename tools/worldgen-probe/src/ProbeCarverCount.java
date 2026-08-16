import java.lang.reflect.Method;
import java.util.Locale;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Holder;
import net.minecraft.core.HolderGetter;
import net.minecraft.core.RegistryAccess;
import net.minecraft.core.registries.Registries;
import net.minecraft.data.registries.VanillaRegistries;
import net.minecraft.server.MinecraftServer;
import net.minecraft.util.RandomSource;
import net.minecraft.world.level.ChunkPos;
import net.minecraft.world.level.biome.Biome;
import net.minecraft.world.level.biome.BiomeGenerationSettings;
import net.minecraft.world.level.block.Blocks;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.chunk.CarvingMask;
import net.minecraft.world.level.chunk.LevelChunkSection;
import net.minecraft.world.level.chunk.ProtoChunk;
import net.minecraft.world.level.levelgen.Aquifer;
import net.minecraft.world.level.levelgen.DensityFunction;
import net.minecraft.world.level.levelgen.Heightmap;
import net.minecraft.world.level.levelgen.LegacyRandomSource;
import net.minecraft.world.level.levelgen.NoiseBasedChunkGenerator;
import net.minecraft.world.level.levelgen.NoiseGeneratorSettings;
import net.minecraft.world.level.levelgen.RandomState;
import net.minecraft.world.level.levelgen.WorldgenRandom;
import net.minecraft.world.level.levelgen.carver.CarvingContext;
import net.minecraft.world.level.levelgen.carver.ConfiguredWorldCarver;
import net.minecraft.world.level.levelgen.synth.NormalNoise;
import net.minecraft.world.level.material.FluidState;

/**
 * Count how many blocks classic carvers would open in chunk (6,-2) seed 12345
 * when starting from solid stone. Uses isStartChunk + carve with a solid ProtoChunk.
 *
 * Simplified: replays applyCarvers seed loop and counts isStartChunk hits +
 * attempts to run carvers if reflection allows.
 */
public class ProbeCarverCount {
    public static void main(String[] args) throws Exception {
        long seed = 12345L;
        int tcx = 6, tcz = -2;
        net.minecraft.SharedConstants.tryDetectVersion();
        net.minecraft.server.Bootstrap.bootStrap();

        // Replay isStartChunk RNG only and print starts + first cave params
        int starts = 0;
        for (int dz = -8; dz <= 8; dz++) {
            for (int dx = -8; dx <= 8; dx++) {
                int scx = tcx + dx, scz = tcz + dz;
                for (int index = 0; index < 2; index++) {
                    WorldgenRandom rng = new WorldgenRandom(new LegacyRandomSource(0L));
                    rng.setLargeFeatureSeed(seed + index, scx, scz);
                    float f = rng.nextFloat();
                    float p = index == 0 ? 0.15f : 0.07f;
                    if (f <= p) {
                        starts++;
                        // peek caveCount
                        int a = rng.nextInt(15) + 1;
                        int b = rng.nextInt(a) + 1;
                        int caveCount = rng.nextInt(b);
                        if (Math.abs(dx) + Math.abs(dz) <= 2) {
                            System.out.printf(Locale.ROOT,
                                "START source=(%d,%d) idx=%d f=%.4f caveCount=%d dist=%d%n",
                                scx, scz, index, f, caveCount, Math.abs(dx)+Math.abs(dz));
                        }
                    }
                }
            }
        }
        System.out.println("total_starts=" + starts);

        // Also verify nextFloat stream matches for one known start (6,-1)
        WorldgenRandom rng = new WorldgenRandom(new LegacyRandomSource(0L));
        rng.setLargeFeatureSeed(seed, 6, -1);
        System.out.printf(Locale.ROOT, "java (6,-1) nextFloat=%.8f%n", rng.nextFloat());
    }
}
