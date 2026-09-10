import java.util.Locale;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Holder;
import net.minecraft.world.level.LevelHeightAccessor;
import net.minecraft.world.level.NoiseColumn;
import net.minecraft.world.level.biome.Biome;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.levelgen.NoiseBasedChunkGenerator;
import net.minecraft.world.level.levelgen.NoiseGeneratorSettings;
import net.minecraft.world.level.levelgen.RandomState;
import net.minecraft.world.level.levelgen.Heightmap;

/**
 * RuinedPortal findSuitableY oracle: raw-noise heights and column solidity
 * around seed 424242 anchor chunk (8,2), giant_portal_2, rotation NONE,
 * mirror FRONT_BACK → bbox x118..127, z32..47, center column (122,39).
 * Mirrors NoiseBasedChunkGenerator.getBaseHeight / getBaseColumn semantics
 * (iterateNoiseColumn writes NO surface rules in 26.2).
 */
public class ProbeRuinPortalBase {
    static final LevelHeightAccessor HEIGHT = new LevelHeightAccessor() {
        @Override public int getHeight() { return 384; }
        @Override public int getMinY() { return -64; }
    };

    public static void main(String[] args) throws Exception {
        long seed = args.length > 0 ? Long.parseLong(args[0]) : 424242L;
        net.minecraft.SharedConstants.tryDetectVersion();
        net.minecraft.server.Bootstrap.bootStrap();
        var lookup = net.minecraft.data.registries.VanillaRegistries.createLookup();
        var noises = lookup.lookupOrThrow(net.minecraft.core.registries.Registries.NOISE);
        var settingsHolder = lookup.lookupOrThrow(net.minecraft.core.registries.Registries.NOISE_SETTINGS)
                .getOrThrow(NoiseGeneratorSettings.OVERWORLD);
        RandomState rs = RandomState.create(settingsHolder.value(), noises, seed);
        var plReg = lookup.lookupOrThrow(net.minecraft.core.registries.Registries.MULTI_NOISE_BIOME_SOURCE_PARAMETER_LIST);
        var plKey = net.minecraft.resources.ResourceKey.create(
                net.minecraft.core.registries.Registries.MULTI_NOISE_BIOME_SOURCE_PARAMETER_LIST,
                net.minecraft.resources.Identifier.parse("minecraft:overworld"));
        var biomeSource = new net.minecraft.world.level.biome.FixedBiomeSource(
                lookup.lookupOrThrow(net.minecraft.core.registries.Registries.BIOME)
                        .getOrThrow(net.minecraft.world.level.biome.Biomes.PLAINS));
        NoiseBasedChunkGenerator gen = new NoiseBasedChunkGenerator(biomeSource, settingsHolder);

        int[][] ptsXZ = {{122,39},{118,32},{127,32},{118,47},{127,47}};
        System.out.printf(Locale.ROOT, "PROBE seed=%d%n", seed);
        for (int[] p : ptsXZ) {
            int hWS = gen.getBaseHeight(p[0], p[1], Heightmap.Types.WORLD_SURFACE_WG, HEIGHT, rs);
            int hOF = gen.getBaseHeight(p[0], p[1], Heightmap.Types.OCEAN_FLOOR_WG, HEIGHT, rs);
            System.out.printf(Locale.ROOT, "col (%d,%d) WS_top=%d OF_top=%d%n", p[0], p[1], hWS - 1, hOF - 1);
        }
        // Corner solidity sweep 30..70 as vanilla's findSuitableY sees it
        // (NOT_AIR for underground placement).
        int[][] corners = {{118,32},{127,32},{118,47},{127,47}};
        for (int y = 65; y >= 30; y--) {
            StringBuilder sb = new StringBuilder();
            int solid = 0;
            sb.append("y=").append(y).append(" : ");
            for (int[] c : corners) {
                NoiseColumn col = gen.getBaseColumn(c[0], c[1], HEIGHT, rs);
                BlockState st = col.getBlock(y);
                boolean na = !st.isAir();
                if (na) solid++;
                sb.append(na ? '#' : '.');
            }
            System.out.printf(Locale.ROOT, "CORNER %s solid=%d%n", sb, solid);
        }
    }
}
