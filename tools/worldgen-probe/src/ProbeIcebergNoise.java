import net.minecraft.SharedConstants;
import net.minecraft.server.Bootstrap;
import net.minecraft.core.registries.Registries;
import net.minecraft.data.registries.VanillaRegistries;
import net.minecraft.resources.Identifier;
import net.minecraft.resources.ResourceKey;
import net.minecraft.world.level.levelgen.NoiseGeneratorSettings;
import net.minecraft.world.level.levelgen.RandomState;
import net.minecraft.world.level.levelgen.synth.NormalNoise;
// Dumps vanilla iceberg noises at block coords: args seed x z [x z ...]
public class ProbeIcebergNoise {
   static ResourceKey<NormalNoise.NoiseParameters> key(String name) {
      return ResourceKey.create(Registries.NOISE, Identifier.withDefaultNamespace(name));
   }
   public static void main(String[] args) throws Exception {
      long seed = Long.parseLong(args[0]);
      SharedConstants.tryDetectVersion();
      Bootstrap.bootStrap();
      var lookup = VanillaRegistries.createLookup();
      var noises = lookup.lookupOrThrow(Registries.NOISE);
      var settings = lookup.lookupOrThrow(Registries.NOISE_SETTINGS).getOrThrow(NoiseGeneratorSettings.OVERWORLD);
      RandomState rs = RandomState.create(settings.value(), noises, seed);
      NormalNoise surface = rs.getOrCreateNoise(key("iceberg_surface"));
      NormalNoise pillar = rs.getOrCreateNoise(key("iceberg_pillar"));
      NormalNoise roof = rs.getOrCreateNoise(key("iceberg_pillar_roof"));
      for (int i = 1; i + 1 < args.length; i += 2) {
         int x = Integer.parseInt(args[i]);
         int z = Integer.parseInt(args[i + 1]);
         double s = Math.abs(surface.getValue(x, 0.0, z) * 8.25);
         double p = Math.abs(pillar.getValue(x * 1.28, 0.0, z * 1.28) * 15.0);
         double berg = Math.min(s, p);
         double r = Math.abs(roof.getValue(x * 1.17, 0.0, z * 1.17) * 1.5);
         double top = Math.min(berg * berg * 1.2, Math.ceil(r * 40.0) + 14.0);
         System.out.printf("x=%d z=%d surface*8.25=%.4f pillar*15=%.4f berg=%.4f roof*1.5=%.4f topRaw=%.4f fires=%s%n",
            x, z, s, p, berg, r, top, berg > 1.8);
      }
   }
}
