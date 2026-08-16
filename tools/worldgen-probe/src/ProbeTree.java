import java.lang.reflect.Field;
import java.util.ArrayList;
import java.util.List;
import java.util.Locale;
import net.minecraft.core.HolderGetter;
import net.minecraft.core.registries.Registries;
import net.minecraft.data.registries.VanillaRegistries;
import net.minecraft.world.level.levelgen.DensityFunction;
import net.minecraft.world.level.levelgen.NoiseGeneratorSettings;
import net.minecraft.world.level.levelgen.NoiseRouter;
import net.minecraft.world.level.levelgen.RandomState;
import net.minecraft.world.level.levelgen.synth.NormalNoise;

public class ProbeTree {
    static List<DensityFunction> dfs(Object o) throws Exception {
        List<DensityFunction> out = new ArrayList<>();
        for (Field f : o.getClass().getDeclaredFields()) {
            if (DensityFunction.class.isAssignableFrom(f.getType())) {
                f.setAccessible(true);
                out.add((DensityFunction) f.get(o));
            }
        }
        return out;
    }
    static void dump(DensityFunction f, String indent, int depth) throws Exception {
        if (depth > 6) return;
        System.out.println(indent + f.getClass().getSimpleName());
        List<DensityFunction> kids = dfs(f);
        for (DensityFunction k : kids) dump(k, indent + "  ", depth + 1);
    }
    public static void main(String[] args) throws Exception {
        long seed = Long.parseLong(args[0]);
        net.minecraft.SharedConstants.tryDetectVersion();
        net.minecraft.server.Bootstrap.bootStrap();
        var lookup = VanillaRegistries.createLookup();
        HolderGetter<NormalNoise.NoiseParameters> noises = lookup.lookupOrThrow(Registries.NOISE);
        var settings = lookup.lookupOrThrow(Registries.NOISE_SETTINGS).getOrThrow(NoiseGeneratorSettings.OVERWORLD);
        RandomState rs = RandomState.create(settings.value(), noises, seed);
        dump(rs.router().finalDensity(), "", 0);
    }
}
