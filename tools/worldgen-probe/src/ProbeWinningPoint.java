import net.minecraft.SharedConstants;
import net.minecraft.core.Holder;
import net.minecraft.core.registries.Registries;
import net.minecraft.data.registries.VanillaRegistries;
import net.minecraft.resources.ResourceKey;
import net.minecraft.server.Bootstrap;
import net.minecraft.world.level.biome.Biome;
import net.minecraft.world.level.biome.Climate;
import net.minecraft.world.level.biome.MultiNoiseBiomeSource;
import net.minecraft.world.level.levelgen.NoiseGeneratorSettings;
import net.minecraft.world.level.levelgen.RandomState;
import com.mojang.datafixers.util.Pair;
import java.io.BufferedReader;
import java.io.InputStreamReader;

public class ProbeWinningPoint {
    public static void main(String[] args) throws Exception {
        long seed = Long.parseLong(args[0]);
        SharedConstants.tryDetectVersion();
        Bootstrap.bootStrap();
        var lookup = VanillaRegistries.createLookup();
        var noises = lookup.lookupOrThrow(Registries.NOISE);
        var settings = lookup.lookupOrThrow(Registries.NOISE_SETTINGS).getOrThrow(NoiseGeneratorSettings.OVERWORLD);
        RandomState rs = RandomState.create(settings.value(), noises, seed);
        var sampler = rs.sampler();
        var registry = lookup.lookupOrThrow(Registries.MULTI_NOISE_BIOME_SOURCE_PARAMETER_LIST);
        var source = MultiNoiseBiomeSource.createFromPreset(
            registry.getOrThrow(ID()));
        // brute-force winning point over the FULL parameter list (list order, strict <)
        var pm = net.minecraft.world.level.biome.MultiNoiseBiomeSource.class.getDeclaredMethod("parameters");
        pm.setAccessible(true);
        @SuppressWarnings("unchecked")
        var list = (Climate.ParameterList<Holder<Biome>>) pm.invoke(source);
        BufferedReader in = new BufferedReader(new InputStreamReader(System.in));
        String line;
        while ((line = in.readLine()) != null) {
            line = line.trim();
            if (line.isEmpty()) continue;
            String[] p = line.split("\\s+");
            int x = Integer.parseInt(p[0]);
            int y = Integer.parseInt(p[1]);
            int z = Integer.parseInt(p[2]);
            Climate.TargetPoint t = sampler.sample(x >> 2, y >> 2, z >> 2);
            long best = Long.MAX_VALUE;
            Climate.ParameterPoint bp = null;
            ResourceKey<Biome> bb = null;
            for (Pair<Climate.ParameterPoint, Holder<Biome>> e : list.values()) {
                long f = fitnessOf(e.getFirst(), t);
                if (f < best) { best = f; bp = e.getFirst(); bb = e.getSecond().unwrapKey().orElseThrow(); }
            }
            System.out.printf("WIN %d %d %d fit=%d biome=%s t=[%d,%d] h=[%d,%d] c=[%d,%d] e=[%d,%d] d=[%d,%d] w=[%d,%d]%n",
                x, y, z, best, bb.identifier(),
                bp.temperature().min(), bp.temperature().max(),
                bp.humidity().min(), bp.humidity().max(),
                bp.continentalness().min(), bp.continentalness().max(),
                bp.erosion().min(), bp.erosion().max(),
                bp.depth().min(), bp.depth().max(),
                bp.weirdness().min(), bp.weirdness().max());
        }
    }
    static long fitnessOf(Climate.ParameterPoint p, Climate.TargetPoint t) throws Exception {
        var m = Climate.ParameterPoint.class.getDeclaredMethod("fitness", Climate.TargetPoint.class);
        m.setAccessible(true);
        return (Long) m.invoke(p, t);
    }
    static ResourceKey<net.minecraft.world.level.biome.MultiNoiseBiomeSourceParameterList> ID() {
        return net.minecraft.world.level.biome.MultiNoiseBiomeSourceParameterLists.OVERWORLD;
    }
}
