import net.minecraft.SharedConstants;
import net.minecraft.core.HolderGetter;
import net.minecraft.core.registries.Registries;
import net.minecraft.data.registries.VanillaRegistries;
import net.minecraft.resources.Identifier;
import net.minecraft.server.Bootstrap;
import net.minecraft.world.level.levelgen.NoiseGeneratorSettings;
import net.minecraft.world.level.levelgen.PositionalRandomFactory;
import net.minecraft.world.level.levelgen.RandomState;
import net.minecraft.world.level.levelgen.synth.NormalNoise;
import net.minecraft.util.RandomSource;

/** Print the vanilla fromHashOf("minecraft:aquifer_fluid_level_floodedness") seed
 *  (via reflection on the Xoroshiro128PlusPlus fields). */
public class ProbeNoiseSeed {
    public static void main(String[] args) throws Exception {
        long seed = Long.parseLong(args[0]);
        SharedConstants.tryDetectVersion();
        Bootstrap.bootStrap();
        var lookup = VanillaRegistries.createLookup();
        HolderGetter<NormalNoise.NoiseParameters> noises = lookup.lookupOrThrow(Registries.NOISE);
        var settings = lookup.lookupOrThrow(Registries.NOISE_SETTINGS).getOrThrow(NoiseGeneratorSettings.OVERWORLD);
        RandomState rs = RandomState.create(settings.value(), noises, seed);
        java.lang.reflect.Field rf = RandomState.class.getDeclaredField("random");
        rf.setAccessible(true);
        PositionalRandomFactory f = (PositionalRandomFactory) rf.get(rs);
        RandomSource rng = f.fromHashOf(Identifier.parse("minecraft:aquifer_fluid_level_floodedness"));
        // reflect into XoroshiroRandomSource.randomNumberGenerator (Xoroshiro128PlusPlus).seedLo/seedHi
        Object gen = rng;
        try {
            java.lang.reflect.Field g = rng.getClass().getDeclaredField("randomNumberGenerator");
            g.setAccessible(true);
            gen = g.get(rng);
            java.lang.reflect.Field lo = gen.getClass().getDeclaredField("seedLo");
            java.lang.reflect.Field hi = gen.getClass().getDeclaredField("seedHi");
            lo.setAccessible(true); hi.setAccessible(true);
            System.out.println("noiseSeedLo=" + Long.toUnsignedString((Long) lo.get(gen)));
            System.out.println("noiseSeedHi=" + Long.toUnsignedString((Long) hi.get(gen)));
        } catch (Exception e) {
            System.out.println("reflect failed: " + e + " class=" + gen.getClass());
        }
    }
}