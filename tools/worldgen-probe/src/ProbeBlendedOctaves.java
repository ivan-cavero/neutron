import java.lang.reflect.Field;
import java.util.Locale;
import net.minecraft.world.level.levelgen.synth.BlendedNoise;
import net.minecraft.world.level.levelgen.synth.ImprovedNoise;
import net.minecraft.world.level.levelgen.synth.PerlinNoise;

/** Dump the three PerlinNoise octave states of BlendedNoise. */
public class ProbeBlendedOctaves {
    static Object get(Object o, String n) throws Exception {
        Field f = o.getClass().getDeclaredField(n);
        f.setAccessible(true);
        return f.get(o);
    }
    static void dump(String label, PerlinNoise pn) throws Exception {
        ImprovedNoise[] levels = (ImprovedNoise[]) get(pn, "noiseLevels");
        int firstOctave = (int) get(pn, "firstOctave");
        System.out.printf("%s", String.format(Locale.ROOT, "%s firstOctave=%d levels=%d%n", label, firstOctave, levels.length));
        for (int i = 0; i < levels.length; i++) {
            if (levels[i] == null) {
                System.out.printf("%s", String.format(Locale.ROOT, "  [%d] octave=%d null%n", i, firstOctave + i));
            } else {
                System.out.printf("%s", String.format(Locale.ROOT, "  [%d] octave=%d xo=%.17g yo=%.17g zo=%.17g%n", i, firstOctave + i, levels[i].xo, levels[i].yo, levels[i].zo));
            }
        }
    }
    public static void main(String[] args) throws Exception {
        net.minecraft.SharedConstants.tryDetectVersion();
        net.minecraft.server.Bootstrap.bootStrap();
        BlendedNoise bn = BlendedNoise.createUnseeded(0.25, 0.125, 80.0, 160.0, 8.0);
        dump("minLimit", (PerlinNoise) get(bn, "minLimitNoise"));
        dump("maxLimit", (PerlinNoise) get(bn, "maxLimitNoise"));
        dump("main", (PerlinNoise) get(bn, "mainNoise"));
    }
}
