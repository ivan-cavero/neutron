import java.lang.reflect.Field;
import java.util.ArrayList;
import java.util.List;

import net.minecraft.core.HolderGetter;
import net.minecraft.core.registries.Registries;
import net.minecraft.data.registries.VanillaRegistries;
import net.minecraft.world.level.levelgen.NoiseGeneratorSettings;
import net.minecraft.world.level.levelgen.RandomState;
import net.minecraft.world.level.levelgen.XoroshiroRandomSource;
import net.minecraft.world.level.levelgen.synth.ImprovedNoise;
import net.minecraft.world.level.levelgen.synth.NormalNoise;
import net.minecraft.world.level.levelgen.synth.PerlinNoise;
import net.minecraft.util.RandomSource;

/**
 * Dump the exact noise states created by a real RandomState for a seed.
 * Verifies the Rust noise-core rewrite reproduces identical ImprovedNoise states.
 */
public class ProbeNoises {

    static final String[] NOISE_KEYS = {
        "clay_bands_offset", "surface", "surface_secondary",
        "badlands_pillar", "badlands_pillar_roof", "badlands_surface",
        "iceberg_pillar", "iceberg_pillar_roof", "iceberg_surface",
        "aquifer_barrier", "aquifer_fluid_level_floodedness", "aquifer_fluid_level_spread",
        "aquifer_lava", "offset", "temperature", "vegetation",
        "continentalness", "erosion", "ridge", "jagged",
        "cave_entrance", "spaghetti_roughness_modulator", "spaghetti_roughness",
        "spaghetti_3d_rarity", "spaghetti_3d_1", "spaghetti_3d_2",
        "spaghetti_3d_thickness", "cave_layer", "cave_cheese",
        "spaghetti_2d_modulator", "spaghetti_2d", "spaghetti_2d_thickness",
        "spaghetti_2d_elevation", "pillar", "pillar_rareness", "pillar_thickness",
        "noodle", "noodle_thickness", "noodle_ridge_a", "noodle_ridge_b",
        "ore_veininess", "ore_vein_a", "ore_vein_b", "ore_gap"
    };

    static Object getField(Object obj, String name) throws Exception {
        Field f = obj.getClass().getDeclaredField(name);
        f.setAccessible(true);
        return f.get(obj);
    }

    static void dumpPerlin(PerlinNoise pn, String label) throws Exception {
        ImprovedNoise[] levels = (ImprovedNoise[]) getField(pn, "noiseLevels");
        int firstOctave = (int) getField(pn, "firstOctave");
        System.out.printf("%s", String.format(java.util.Locale.ROOT, "%s: firstOctave=%d levels=%d%n", label, firstOctave, levels.length));
        for (int i = 0; i < levels.length; i++) {
            ImprovedNoise in = levels[i];
            if (in == null) {
                System.out.printf("%s", String.format(java.util.Locale.ROOT, "  [%d] octave=%d null%n", i, firstOctave + i));
            } else {
                System.out.printf("%s", String.format(java.util.Locale.ROOT, "  [%d] octave=%d xo=%.17g yo=%.17g zo=%.17g%n",
                    i, firstOctave + i, in.xo, in.yo, in.zo));
            }
        }
    }

    static void dumpNoise(RandomState rs, String key) throws Exception {
        var holder = rs.getOrCreateNoise(
            net.minecraft.resources.ResourceKey.create(
                net.minecraft.core.registries.Registries.NOISE,
                net.minecraft.resources.Identifier.withDefaultNamespace(key)));
        // key create: use reflection-free path via Noises? Simpler: get from registry
        System.out.printf("%s", String.format(java.util.Locale.ROOT, "== noise %s ==%n", key));
        NormalNoise nn = holder;
        PerlinNoise first = (PerlinNoise) getField(nn, "first");
        PerlinNoise second = (PerlinNoise) getField(nn, "second");
        dumpPerlin(first, "  first");
        dumpPerlin(second, "  second");
        // sample values
        for (double[] c : new double[][]{{0, 0, 0}, {100.5, 40, 200.5}, {-57, 63, 31}}) {
            System.out.printf("%s", String.format(java.util.Locale.ROOT, "  sample(%.1f,%.1f,%.1f) = %.17g%n", c[0], c[1], c[2], nn.getValue(c[0], c[1], c[2])));
        }
    }

    public static void main(String[] args) throws Exception {
        long seed = Long.parseLong(args[0]);
        net.minecraft.SharedConstants.tryDetectVersion();
        net.minecraft.server.Bootstrap.bootStrap();
        var lookup = VanillaRegistries.createLookup();
        HolderGetter<NormalNoise.NoiseParameters> noises = lookup.lookupOrThrow(Registries.NOISE);
        var settings = lookup.lookupOrThrow(Registries.NOISE_SETTINGS).getOrThrow(NoiseGeneratorSettings.OVERWORLD);
        RandomState rs = RandomState.create(settings.value(), noises, seed);

        // Main positional factory seed (lo, hi)
        RandomSource rng = settings.value().getRandomSource().newInstance(seed);
        var pos = rng.forkPositional();
        Field loF = pos.getClass().getDeclaredField("seedLo");
        Field hiF = pos.getClass().getDeclaredField("seedHi");
        loF.setAccessible(true); hiF.setAccessible(true);
        System.out.printf("%s", String.format(java.util.Locale.ROOT, "seed=%d mainPosLo=%d mainPosHi=%d%n", seed, (long) loF.get(pos), (long) hiF.get(pos)));

        for (String key : NOISE_KEYS) {
            dumpNoise(rs, key);
        }
    }
}
