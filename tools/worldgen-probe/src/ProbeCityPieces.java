import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import java.util.Locale;
import java.util.Optional;
import net.minecraft.SharedConstants;
import net.minecraft.server.Bootstrap;
import net.minecraft.server.packs.PackResources;
import net.minecraft.server.packs.PackType;
import net.minecraft.server.packs.PathPackResources;
import net.minecraft.server.packs.repository.Pack;
import net.minecraft.server.packs.repository.PackCompatibility;
import net.minecraft.server.packs.repository.PackSource;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Holder;
import net.minecraft.core.HolderLookup;
import net.minecraft.core.registries.Registries;
import net.minecraft.data.registries.VanillaRegistries;
import net.minecraft.world.level.ChunkPos;
import net.minecraft.world.level.LevelHeightAccessor;
import net.minecraft.world.level.biome.Biome;
import net.minecraft.world.level.biome.Biomes;
import net.minecraft.world.level.biome.FixedBiomeSource;
import net.minecraft.world.level.levelgen.WorldOptions;
import net.minecraft.world.level.levelgen.RandomState;
import net.minecraft.world.level.levelgen.structure.Structure;
import net.minecraft.world.level.levelgen.structure.structures.JigsawStructure;
import net.minecraft.world.level.levelgen.structure.templatesystem.StructureTemplateManager;
import net.minecraft.world.level.storage.LevelStorageSource;

/** Dumps vanilla ancient_city jigsaw pieces (BB + template + rotation) for seed/chunk. */
public class ProbeCityPieces {
    public static void main(String[] args) throws Exception {
        long seed = Long.parseLong(args[0]);
        int cx = Integer.parseInt(args[1]);
        int cz = Integer.parseInt(args[2]);
        SharedConstants.tryDetectVersion();
        Bootstrap.bootStrap();
        HolderLookup.Provider lookup = VanillaRegistries.createLookup();

        // 1. resource manager over an extracted jar data/ tree
        Path root = Files.createTempDirectory("cityprobe");
        Path dataDir = root.resolve("data");
        extractJarData(dataDir);
        PathPackResources pack = new PathPackResources(
            new net.minecraft.server.packs.PackLocationInfo("cityprobe",
                net.minecraft.network.chat.Component.literal("cityprobe"),
                PackSource.BUILT_IN, java.util.Optional.empty()),
            root);
        net.minecraft.server.packs.resources.ResourceManager rm =
            new net.minecraft.server.packs.resources.MultiPackResourceManager(
                PackType.SERVER_DATA, List.of(pack));

        // 2. template manager with a storage access stub for GENERATED_DIR
        Path worldDir = Files.createTempDirectory("cityworld");
        LevelStorageSource.LevelStorageAccess storage =
            LevelStorageSource.createDefault(worldDir.getParent())
                .createAccess(worldDir.getFileName().toString());
        StructureTemplateManager templates = new StructureTemplateManager(
            rm, storage, net.minecraft.util.datafix.DataFixers.getDataFixer(),
            lookup.lookupOrThrow(Registries.BLOCK));

        // 3. generation context
        var settingsHolder = lookup.lookupOrThrow(Registries.NOISE_SETTINGS)
            .getOrThrow(net.minecraft.world.level.levelgen.NoiseGeneratorSettings.OVERWORLD);
        var settings = settingsHolder.value();
        Holder<Biome> plains = lookup.lookupOrThrow(Registries.BIOME).getOrThrow(Biomes.PLAINS);
        var biomeSource = new FixedBiomeSource(plains);
        var gen = new net.minecraft.world.level.levelgen.NoiseBasedChunkGenerator(
            biomeSource, settingsHolder);
        RandomState rs = RandomState.create(settings,
            lookup.lookupOrThrow(Registries.NOISE), seed);
        LevelHeightAccessor lha = new LevelHeightAccessor() {
            @Override public int getMinY() { return -64; }
            @Override public int getHeight() { return 384; }
            @Override public int getMaxY() { return 320; }
            @Override public boolean isOutsideBuildHeight(int y) { return y < -64 || y >= 320; }
            @Override public int getMinSectionY() { return -4; }
            @Override public int getMaxSectionY() { return 19; }
            @Override public int getSectionsCount() { return 24; }
            @Override public int getSectionIndex(int y) { return (y >> 4) + 4; }
            @Override public int getSectionIndexFromSectionY(int sy) { return sy + 4; }
        };
        // Load the WORLDGEN datapack registries (template pools, structures,
        // configured/placed features...) from the extracted pack.
        java.util.concurrent.Executor exec = Runnable::run;
        // Only the registries jigsaw assembly needs (keeps load fast + quiet).
        java.util.List<net.minecraft.resources.RegistryDataLoader.RegistryData<?>> needed =
            new java.util.ArrayList<>();
        java.util.Set<String> skip = java.util.Set.of(
            "cat_sound_variant", "cat_variant", "chicken_sound_variant", "chicken_variant",
            "cow_sound_variant", "cow_variant", "damage_type", "dialog", "enchantment",
            "enchantment_provider", "frog_variant", "instrument", "jukebox_song",
            "painting_variant", "pig_sound_variant", "pig_variant", "banner_pattern",
            "trade_set", "trim_material", "trim_pattern", "villager_trade",
            "wolf_sound_variant", "wolf_variant", "zombie_nautilus_variant",
            "sulfur_cube_archetype", "test_environment", "test_instance", "timeline",
            "world_clock", "chat_type", "flat_level_generator_preset",
            "multi_noise_biome_source_parameter_list", "dimension_type", "world_preset");
        for (var rd : net.minecraft.resources.RegistryDataLoader.WORLDGEN_REGISTRIES) {
            String p = rd.key().identifier().getPath()
                .replaceFirst("^worldgen/", "");
            if (!skip.contains(p)) {
                needed.add(rd);
            }
        }
        net.minecraft.core.RegistryAccess.Frozen regAccess = net.minecraft.resources.RegistryDataLoader
            .load(rm, contextRegistries(lookup), needed, exec)
            .join();
        System.out.println("LOADED-REGISTRIES:");
        regAccess.registries().forEach(r -> System.out.println("  " + r.key()));
        // counting random: subclass LegacyRandomSource so WorldgenRandom's
        // `instanceof LegacyRandomSource` fast path applies and the values are
        // bit-identical; only the draw count is tracked.
        class CountingSource extends net.minecraft.world.level.levelgen.LegacyRandomSource {
            long draws = 0;
            CountingSource() { super(0L); }
            @Override public void setSeed(long s) { super.setSeed(s); draws += 1; }
            @Override public int next(int bits) { draws += 1; return super.next(bits); }
        }
        CountingSource counting = new CountingSource();
        net.minecraft.world.level.levelgen.WorldgenRandom wrandom =
            new net.minecraft.world.level.levelgen.WorldgenRandom(counting);
        wrandom.setLargeFeatureSeed(seed, cx, cz);

        Structure.GenerationContext ctx = new Structure.GenerationContext(
            regAccess, gen, biomeSource, rs, templates, wrandom, seed, new ChunkPos(cx, cz),
            lha, b -> true);

        // 4. the ancient_city structure (datapack JSON replicated inline)
        JigsawStructure city = new JigsawStructure(
            structureSettings(),
            lookup.lookupOrThrow(Registries.TEMPLATE_POOL)
                .getOrThrow(net.minecraft.resources.ResourceKey.create(
                    Registries.TEMPLATE_POOL,
                    net.minecraft.resources.Identifier.withDefaultNamespace("ancient_city/city_center"))),
            java.util.Optional.of(net.minecraft.resources.Identifier.withDefaultNamespace("city_anchor")),
            7,
            net.minecraft.world.level.levelgen.heightproviders.ConstantHeight.of(
                net.minecraft.world.level.levelgen.VerticalAnchor.absolute(-27)),
            false,
            java.util.Optional.empty(),
            new JigsawStructure.MaxDistance(116),
            java.util.List.of(),
            JigsawStructure.DEFAULT_DIMENSION_PADDING,
            JigsawStructure.DEFAULT_LIQUID_SETTINGS);

        Optional<Structure.GenerationStub> stub = city.findGenerationPoint(ctx);
        if (stub.isEmpty()) {
            System.out.println("NO-STUB");
            return;
        }
        net.minecraft.world.level.levelgen.structure.pieces.StructurePiecesBuilder builder =
            stub.get().getPiecesBuilder();
        int i = 0;
        for (var piece : builder.build().pieces()) {
            var bb = piece.getBoundingBox();
            String tpl = "?";
            String rot = "?";
            if (piece instanceof net.minecraft.world.level.levelgen.structure.
                    PoolElementStructurePiece pep) {
                var el = pep.getElement();
                if (el instanceof net.minecraft.world.level.levelgen.structure.pools.
                        SinglePoolElement spe) {
                    try {
                        var f = net.minecraft.world.level.levelgen.structure.pools.
                            SinglePoolElement.class.getDeclaredField("template");
                        f.setAccessible(true);
                        @SuppressWarnings("unchecked")
                        Object o = f.get(spe);
                        // template field = Either<ResourceKey, StructureTemplate>
                        if (o instanceof com.mojang.datafixers.util.Either<?, ?> either
                            && either.left().isPresent()) {
                            Object left = either.left().get();
                            tpl = left instanceof net.minecraft.resources.ResourceKey<?> key
                                ? key.identifier().getPath()
                                : left.toString();
                        } else {
                            tpl = "INLINE";
                        }
                    } catch (Exception e) {
                        tpl = "ERR:" + e.getMessage();
                    }
                } else if (el instanceof net.minecraft.world.level.levelgen.structure.pools.
                        FeaturePoolElement) {
                    tpl = "FEATURE";
                } else if (el instanceof net.minecraft.world.level.levelgen.structure.pools.
                        ListPoolElement lpe) {
                    try {
                        var f2 = net.minecraft.world.level.levelgen.structure.pools.
                            ListPoolElement.class.getDeclaredField("elements");
                        f2.setAccessible(true);
                        tpl = "LIST:" + ((java.util.List<?>) f2.get(lpe)).size();
                    } catch (Exception e2) {
                        tpl = "LIST:ERR";
                    }
                } else if (el instanceof net.minecraft.world.level.levelgen.structure.pools.
                        EmptyPoolElement) {
                    tpl = "EMPTY";
                }
                rot = pep.getRotation().toString();
            }
            System.out.printf(Locale.ROOT, "PIECE %d %s %s %d %d %d %d %d %d draws=%d%n",
                i, tpl, rot, bb.minX(), bb.minY(), bb.minZ(),
                bb.maxX(), bb.maxY(), bb.maxZ(), counting.draws);
            i++;
        }
    }

    @SuppressWarnings({"unchecked", "rawtypes"})
    static java.util.List<HolderLookup.RegistryLookup<?>> contextRegistries(HolderLookup.Provider lookup) {
        // every static registry as context (block/fluid/sound_event/... are
        // referenced by worldgen codecs)
        java.util.List<HolderLookup.RegistryLookup<?>> out = new java.util.ArrayList<>();
        lookup.listRegistries().forEach(l -> out.add((HolderLookup.RegistryLookup<?>) l));
        return out;
    }

    static net.minecraft.world.level.levelgen.structure.Structure.StructureSettings structureSettings() {
        return new net.minecraft.world.level.levelgen.structure.Structure.StructureSettings(
            net.minecraft.core.HolderSet.empty(), java.util.Map.of(),
            net.minecraft.world.level.levelgen.GenerationStep.Decoration.UNDERGROUND_DECORATION,
            net.minecraft.world.level.levelgen.structure.TerrainAdjustment.BEARD_BOX);
    }

    static void extractJarData(Path dest) throws Exception {
        java.util.jar.JarFile jar = new java.util.jar.JarFile(
            "../nbt-ref/vanilla-fresh-424242/versions/26.2/server-26.2.jar");
        var entries = jar.entries();
        while (entries.hasMoreElements()) {
            var e = entries.nextElement();
            if (!e.getName().startsWith("data/") || e.isDirectory()) continue;
            Path out = dest.resolve(e.getName().substring("data/".length()));
            Files.createDirectories(out.getParent());
            Files.copy(jar.getInputStream(e), out,
                java.nio.file.StandardCopyOption.REPLACE_EXISTING);
        }
        // pack.mcmeta for the pack loader (at the PACK ROOT, not data/)
        Path meta = dest.getParent().resolve("pack.mcmeta");
        if (!Files.exists(meta)) {
            Files.writeString(meta, "{\"pack\":{\"pack_format\":80,\"description\":\"probe\"}}");
        }
        jar.close();
    }
}
