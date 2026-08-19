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

/** Print the vanilla aquifer positional seed pair (fromHashOf("aquifer").forkPositional()). */
public class ProbeAquiferSeed {
    public static void main(String[] args) throws Exception {
        long seed = Long.parseLong(args[0]);
        SharedConstants.tryDetectVersion();
        Bootstrap.bootStrap();
        var lookup = VanillaRegistries.createLookup();
        HolderGetter<NormalNoise.NoiseParameters> noises = lookup.lookupOrThrow(Registries.NOISE);
        var settings = lookup.lookupOrThrow(Registries.NOISE_SETTINGS).getOrThrow(NoiseGeneratorSettings.OVERWORLD);
        RandomState rs = RandomState.create(settings.value(), noises, seed);
        // aquiferRandom is private; recompute via the same derivation:
        // this.random.fromHashOf(Identifier.withDefaultNamespace("aquifer")).forkPositional()
        // We can't access rs.random() directly, but we can print the aquiferRandom via reflection-free
        // equivalent: the PositionalRandomFactory for a named id.
        // Instead, print the terrain random derivation path by re-deriving from the world seed is not
        // exposed; so print the fromHashOf seed of the aquifer string via RandomSupport directly.
        PositionalRandomFactory f = rs.aquiferRandom();
        // Print the seed pair via reflection (the factory holds seedLo/seedHi privately).
        try {
            java.lang.reflect.Field loF = PositionalRandomFactory.class.getDeclaredField("seedLo");
            java.lang.reflect.Field hiF = PositionalRandomFactory.class.getDeclaredField("seedHi");
            loF.setAccessible(true); hiF.setAccessible(true);
            long lo = (Long) loF.get(f);
            long hi = (Long) hiF.get(f);
            System.out.println("aquifer seedLo=" + Long.toUnsignedString(lo) + " seedHi=" + Long.toUnsignedString(hi));
        } catch (Exception e) {
            System.out.println("reflect failed: " + e);
        }
    }
}