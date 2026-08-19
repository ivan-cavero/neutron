import net.minecraft.SharedConstants;
import net.minecraft.core.Holder;
import net.minecraft.core.Registry;
import net.minecraft.core.registries.Registries;
import net.minecraft.data.registries.VanillaRegistries;
import net.minecraft.resources.Identifier;
import net.minecraft.resources.ResourceKey;
import net.minecraft.server.Bootstrap;
import net.minecraft.world.level.biome.Biome;
import net.minecraft.world.level.biome.Climate;
import net.minecraft.world.level.biome.MultiNoiseBiomeSourceParameterList;

/** Dump the overworld parameter list: one line per point:
 *  biomeName offset tmin tmax hmin hmax cmin cmax emin emax dmin dmax wmin wmax
 *  (intervals and offset are the quantized longs, scale x10000). */
public class ProbeParamsDump {
    public static void main(String[] args) throws Exception {
        SharedConstants.tryDetectVersion();
        Bootstrap.bootStrap();
        var lookup = VanillaRegistries.createLookup();
        var key = ResourceKey.create(Registries.MULTI_NOISE_BIOME_SOURCE_PARAMETER_LIST,
            Identifier.parse("minecraft:overworld"));
        var list = lookup.lookupOrThrow(Registries.MULTI_NOISE_BIOME_SOURCE_PARAMETER_LIST)
            .getOrThrow(key).value().parameters();
        for (var pair : list.values()) {
            Climate.ParameterPoint p = pair.getFirst();
            Holder<Biome> b = pair.getSecond();
            String bn = b.unwrapKey().map(k -> k.identifier().toString().replace("minecraft:", "")).orElse("?");
            System.out.println(bn + " " + p.offset()
                + " " + p.temperature().min() + " " + p.temperature().max()
                + " " + p.humidity().min() + " " + p.humidity().max()
                + " " + p.continentalness().min() + " " + p.continentalness().max()
                + " " + p.erosion().min() + " " + p.erosion().max()
                + " " + p.depth().min() + " " + p.depth().max()
                + " " + p.weirdness().min() + " " + p.weirdness().max());
        }
    }
}
