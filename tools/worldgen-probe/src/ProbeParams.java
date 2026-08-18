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
import net.minecraft.world.level.biome.MultiNoiseBiomeSource;
import net.minecraft.world.level.biome.MultiNoiseBiomeSourceParameterList;
import net.minecraft.world.level.biome.MultiNoiseBiomeSourceParameterLists;

public class ProbeParams {
    public static void main(String[] args) throws Exception {
        SharedConstants.tryDetectVersion();
        Bootstrap.bootStrap();
        var lookup = VanillaRegistries.createLookup();
        var registry = lookup.lookupOrThrow(Registries.MULTI_NOISE_BIOME_SOURCE_PARAMETER_LIST);
        var key = ResourceKey.create(Registries.MULTI_NOISE_BIOME_SOURCE_PARAMETER_LIST, Identifier.parse("minecraft:overworld"));
        var list = registry.getOrThrow(key).value();
        var params = list.parameters();
        for (var pair : params.values()) {
            var point = pair.getFirst();
            var biome = pair.getSecond().unwrapKey().map(k -> k.identifier().toString()).orElse("?");
            if (biome.contains("lush_caves") || biome.contains("pale_garden")) {
                System.out.println("biome=" + biome
                    + " t=[" + point.temperature() + "] h=[" + point.humidity() + "] c=[" + point.continentalness() + "]"
                    + " e=[" + point.erosion() + "] d=[" + point.depth() + "] w=[" + point.weirdness() + "] off=" + point.offset());
            }
        }
    }
}
