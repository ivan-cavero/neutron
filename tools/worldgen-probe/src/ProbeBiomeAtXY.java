import net.minecraft.SharedConstants;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Holder;
import net.minecraft.core.registries.Registries;
import net.minecraft.data.registries.VanillaRegistries;
import net.minecraft.resources.Identifier;
import net.minecraft.resources.ResourceKey;
import net.minecraft.server.Bootstrap;
import net.minecraft.world.level.biome.Biome;
import net.minecraft.world.level.biome.BiomeManager;
import net.minecraft.world.level.biome.MultiNoiseBiomeSource;
import net.minecraft.world.level.biome.MultiNoiseBiomeSourceParameterList;
import net.minecraft.world.level.levelgen.NoiseGeneratorSettings;
import net.minecraft.world.level.levelgen.RandomState;
// Point-query the vanilla climate biome at block (x, y, z): args seed x y z.
public class ProbeBiomeAtXY {
   public static void main(String[] args) throws Exception {
      long seed = Long.parseLong(args[0]);
      int x = Integer.parseInt(args[1]);
      int y = Integer.parseInt(args[2]);
      int z = Integer.parseInt(args[3]);
      SharedConstants.tryDetectVersion();
      Bootstrap.bootStrap();
      var lookup = VanillaRegistries.createLookup();
      var noises = lookup.lookupOrThrow(Registries.NOISE);
      var settings = lookup.lookupOrThrow(Registries.NOISE_SETTINGS).getOrThrow(NoiseGeneratorSettings.OVERWORLD);
      RandomState rs = RandomState.create(settings.value(), noises, seed);
      var registry = lookup.lookupOrThrow(Registries.MULTI_NOISE_BIOME_SOURCE_PARAMETER_LIST);
      var key = ResourceKey.create(Registries.MULTI_NOISE_BIOME_SOURCE_PARAMETER_LIST, Identifier.parse("minecraft:overworld"));
      var source = MultiNoiseBiomeSource.createFromPreset(registry.getOrThrow(key));
      var sampler = rs.sampler();
      BiomeManager mgr = new BiomeManager(new BiomeManager.NoiseBiomeSource() {
         public Holder<Biome> getNoiseBiome(int qx, int qy, int qz) {
            return source.getNoiseBiome(qx, qy, qz, sampler);
         }
      }, BiomeManager.obfuscateSeed(seed));
      for (int yy : new int[]{y, -40, 0, 46, 63}) {
         Holder<Biome> b = mgr.getBiome(new BlockPos(x, yy, z));
         System.out.println("(" + x + "," + yy + "," + z + ") biome=" + b.unwrapKey().map(k -> k.identifier().toString()).orElse("?"));
      }
   }
}
