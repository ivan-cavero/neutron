import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.util.*;
import net.minecraft.core.HolderGetter;
import net.minecraft.core.registries.Registries;
import net.minecraft.data.registries.VanillaRegistries;
import net.minecraft.world.level.levelgen.DensityFunction;
import net.minecraft.world.level.levelgen.NoiseGeneratorSettings;
import net.minecraft.world.level.levelgen.RandomState;
import net.minecraft.world.level.levelgen.synth.NormalNoise;

public class ProbeNoodleTree {
    static void dump(Object f, String indent, int depth) throws Exception {
        if (f == null || depth > 14) {
            System.out.println(indent + "...");
            return;
        }
        String name = f.getClass().getSimpleName();
        String extra = "";
        // try type()/wrapped()/input()/argument1()/argument2()
        for (String m : List.of("type", "kind")) {
            try {
                Method mm = f.getClass().getMethod(m);
                extra += " " + m + "=" + mm.invoke(f);
            } catch (Exception ignored) {}
        }
        System.out.println(indent + name + extra);
        // known child accessors
        for (String m : List.of("wrapped", "input", "argument1", "argument2", "inputA", "inputB")) {
            try {
                Method mm = f.getClass().getMethod(m);
                Object c = mm.invoke(f);
                if (c instanceof DensityFunction) {
                    System.out.println(indent + "  [" + m + "]");
                    dump(c, indent + "    ", depth + 1);
                }
            } catch (Exception ignored) {}
        }
        // fields that are DensityFunction
        for (Field field : f.getClass().getDeclaredFields()) {
            field.setAccessible(true);
            Object v = field.get(f);
            if (v instanceof DensityFunction df) {
                // skip if already printed via accessor
                boolean skip = false;
                for (String m : List.of("wrapped", "input", "argument1", "argument2")) {
                    try {
                        if (f.getClass().getMethod(m).invoke(f) == v) skip = true;
                    } catch (Exception ignored) {}
                }
                if (!skip) {
                    System.out.println(indent + "  ." + field.getName());
                    dump(df, indent + "    ", depth + 1);
                }
            }
        }
    }

    public static void main(String[] args) throws Exception {
        net.minecraft.SharedConstants.tryDetectVersion();
        net.minecraft.server.Bootstrap.bootStrap();
        var lookup = VanillaRegistries.createLookup();
        HolderGetter<NormalNoise.NoiseParameters> noises = lookup.lookupOrThrow(Registries.NOISE);
        var settings = lookup.lookupOrThrow(Registries.NOISE_SETTINGS).getOrThrow(NoiseGeneratorSettings.OVERWORLD);
        RandomState rs = RandomState.create(settings.value(), noises, 12345L);
        DensityFunction fd = rs.router().finalDensity();
        System.out.println("=== FINAL ===");
        dump(fd, "", 0);
        // second arg only (noodle)
        System.out.println("=== NOODLE (arg2) ===");
        Method a2 = fd.getClass().getMethod("argument2");
        dump(a2.invoke(fd), "", 0);
    }
}
