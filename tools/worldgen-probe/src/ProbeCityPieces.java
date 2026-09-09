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
import net.minecraft.world.level.levelgen.structure.pools.StructureTemplatePool;
import net.minecraft.world.level.levelgen.structure.templatesystem.StructureTemplateManager;
import net.minecraft.world.level.storage.LevelStorageSource;

/** Dumps vanilla ancient_city jigsaw pieces (BB + template + rotation) for seed/chunk. */
public class ProbeCityPieces {
    static class CountingSource extends net.minecraft.world.level.levelgen.LegacyRandomSource {
        long draws = 0;
        CountingSource() { super(0L); }
        @Override public void setSeed(long s2) { super.setSeed(s2); draws += 1; }
        @Override public int next(int bits) { draws += 1; return super.next(bits); }
    }

    static HolderLookup.Provider lookup;
    public static void main(String[] args) throws Exception {
        long seed = Long.parseLong(args[0]);
        int cx = Integer.parseInt(args[1]);
        int cz = Integer.parseInt(args[2]);
        SharedConstants.tryDetectVersion();
        Bootstrap.bootStrap();
        lookup = VanillaRegistries.createLookup();

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

        // MiniPlacer: vanilla JigsawPlacement.addPieces copied 1:1 (public
        // APIs only) with draw-count prints at each piece accept. Replaces the
        // findGenerationPoint call so per-piece draw accounting is possible.
        Optional<Structure.GenerationStub> stub = miniAddPieces(city, ctx, regAccess, lha, counting);
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

    // --- MiniPlacer: JigsawPlacement.addPieces copied 1:1 (public APIs) with
    // --- draw-count prints at each accept. Uses ctx.random (the counting
    // --- WorldgenRandom) exactly like vanilla.

    static Optional<Structure.GenerationStub> miniAddPieces(
        JigsawStructure city, Structure.GenerationContext ctx,
        net.minecraft.core.RegistryAccess.Frozen regAccess,
        LevelHeightAccessor lha, CountingSource counting) {
        // replicate JigsawStructure.findGenerationPoint
        // Replicate JigsawStructure.findGenerationPoint here (with draw
        // accounting at each accept).
        ChunkPos chunkPos = ctx.chunkPos();
        // startHeight absolute -27 → no RNG
        BlockPos startPos = new BlockPos(chunkPos.getMinBlockX(), -27, chunkPos.getMinBlockZ());
        var startPoolHolder = regAccess.lookupOrThrow(Registries.TEMPLATE_POOL)
            .getOrThrow(net.minecraft.resources.ResourceKey.create(Registries.TEMPLATE_POOL,
                net.minecraft.resources.Identifier.withDefaultNamespace("ancient_city/city_center")));
        var pools = regAccess.lookupOrThrow(Registries.TEMPLATE_POOL);
        var random = ctx.random();
        var centerRotation = net.minecraft.world.level.block.Rotation.getRandom(random);
        var centerPool = startPoolHolder.value();
        var centerElement = centerPool.getRandomTemplate(random);
        if (centerElement == net.minecraft.world.level.levelgen.structure.pools.EmptyPoolElement.INSTANCE) {
            System.out.println("NO-STUB");
            return Optional.empty();
        }
        // startJigsaw present: city_anchor
        BlockPos anchoredPosition = null;
        for (var jigsaw : centerElement.getShuffledJigsawBlocks(
                ctx.structureTemplateManager(), startPos, centerRotation, random)) {
            if (net.minecraft.resources.Identifier.withDefaultNamespace("city_anchor")
                .equals(jigsaw.name())) {
                anchoredPosition = jigsaw.info().pos();
                break;
            }
        }
        if (anchoredPosition == null) {
            System.out.println("NO-ANCHOR");
            return Optional.empty();
        }
        var localAnchorPosition = anchoredPosition.subtract(startPos);
        var adjustedPosition = startPos.subtract(localAnchorPosition);
        var centerPiece = new net.minecraft.world.level.levelgen.structure.PoolElementStructurePiece(
            ctx.structureTemplateManager(), centerElement, adjustedPosition,
            centerElement.getGroundLevelDelta(), centerRotation,
            centerElement.getBoundingBox(ctx.structureTemplateManager(), adjustedPosition, centerRotation),
            JigsawStructure.DEFAULT_LIQUID_SETTINGS);
        var box = centerPiece.getBoundingBox();
        int centerX = (box.maxX() + box.minX()) / 2;
        int centerZ = (box.maxZ() + box.minZ()) / 2;
        int bottomY = adjustedPosition.getY(); // projectStartToHeightmap empty
        int oldAbsoluteGroundY = box.minY() + centerPiece.getGroundLevelDelta();
        centerPiece.move(0, bottomY - oldAbsoluteGroundY, 0);
        int centerY = bottomY + localAnchorPosition.getY();

        // expansion shape = global AABB minus center box (vanilla VoxelShape)
        var aabb = new net.minecraft.world.phys.AABB(
            centerX - 116,
            Math.max(centerY - 384, lha.getMinY()),
            centerZ - 116,
            centerX + 116 + 1,
            Math.min(centerY + 384 + 1, lha.getMaxY() + 1),
            centerZ + 116 + 1);
        var shape = net.minecraft.world.phys.shapes.Shapes.join(
            net.minecraft.world.phys.shapes.Shapes.create(aabb),
            net.minecraft.world.phys.shapes.Shapes.create(net.minecraft.world.phys.AABB.of(box)),
            net.minecraft.world.phys.shapes.BooleanOp.ONLY_FIRST);

        var pieces = new java.util.ArrayList<Object[]>();
        pieces.add(new Object[]{centerPiece, 0});
        // Use MutableObject equivalent: a one-element array
        var freeHolder = new Object[]{ shape };
        try {
            var placerCls = Class.forName("net.minecraft.world.level.levelgen.structure.pools.JigsawPlacement");
            System.out.println("MINI-PLACE-START");
            miniTryPlacingChildren(ctx, regAccess, pieces, centerPiece, 0, freeHolder, 0, lha, random, counting);
            // FIFO drain
            int idx = 1;
            while (idx < pieces.size()) {
                var pair0 = (Object[]) pieces.get(idx);
                var st = (net.minecraft.world.level.levelgen.structure.PoolElementStructurePiece) pair0[0];
                int stDepth = (Integer) pair0[1];
                if (stDepth <= 7) {
                    miniTryPlacingChildren(ctx, regAccess, pieces, st, stDepth,
                        freeHolder, stDepth, lha, random, counting);
                }
                idx++;
            }
        } catch (Exception e) {
            e.printStackTrace();
        }
        var stub = new Structure.GenerationStub(
            new BlockPos(centerX, centerY, centerZ),
            (net.minecraft.world.level.levelgen.structure.pieces.StructurePiecesBuilder b) ->
                pieces.forEach(pd -> {
                    var pair = (Object[]) pd;
                    b.addPiece((net.minecraft.world.level.levelgen.structure.PoolElementStructurePiece) pair[0]);
                }));
        return Optional.of(stub);
    }

    static void miniTryPlacingChildren(
        Structure.GenerationContext ctx,
        net.minecraft.core.RegistryAccess.Frozen regAccess,
        java.util.List<Object[]> pieces,
        net.minecraft.world.level.levelgen.structure.PoolElementStructurePiece sourcePiece,
        int sourceDepth,
        Object[] contextFree,
        int depth,
        LevelHeightAccessor heightAccessor,
        net.minecraft.util.RandomSource random,
        CountingSource counting) {
        var sourceElement = sourcePiece.getElement();
        var sourceBoxPosition = sourcePiece.getPosition();
        var sourceRotation = sourcePiece.getRotation();
        var sourceProjection = sourceElement.getProjection();
        boolean sourceRigid = sourceProjection == net.minecraft.world.level.levelgen.structure.pools.StructureTemplatePool.Projection.RIGID;
        var sourceFree = new Object[]{ null };
        var sourceBB = sourcePiece.getBoundingBox();
        int sourceBoxY = sourceBB.minY();
        var pools = regAccess.lookupOrThrow(Registries.TEMPLATE_POOL);

        for (var sourceJigsaw : sourceElement.getShuffledJigsawBlocks(
                ctx.structureTemplateManager(), sourceBoxPosition, sourceRotation, random)) {
            var sourceJigsawInfo = sourceJigsaw.info();
            var sourceDirection = net.minecraft.world.level.block.JigsawBlock.getFrontFacing(sourceJigsawInfo.state());
            var sourceJigsawPos = sourceJigsawInfo.pos();
            var targetJigsawPos = sourceJigsawPos.relative(sourceDirection);
            int sourceJigsawLocalY = sourceJigsawPos.getY() - sourceBoxY;
            int sourceJigsawBaseHeight = Integer.MIN_VALUE;
            var poolName = sourceJigsaw.pool();
            Optional<Holder.Reference<StructureTemplatePool>> maybeTargetPool = pools.get(poolName);
            if (maybeTargetPool.isEmpty()) continue;
            var targetPool = maybeTargetPool.get();
            if (targetPool.value().size() == 0
                && !targetPool.is(net.minecraft.resources.ResourceKey.create(
                    Registries.TEMPLATE_POOL,
                    net.minecraft.resources.Identifier.withDefaultNamespace("empty")))) continue;
            var fallback = targetPool.value().getFallback();
            boolean attachInsideSource = sourceBB.isInside(targetJigsawPos);
            Object[] childrenFree;
            if (attachInsideSource) {
                childrenFree = sourceFree;
                if (sourceFree[0] == null) {
                    sourceFree[0] = net.minecraft.world.phys.shapes.Shapes.create(
                        net.minecraft.world.phys.AABB.of(sourceBB));
                }
            } else {
                childrenFree = contextFree;
            }
            var targetPieces = new java.util.ArrayList<net.minecraft.world.level.levelgen.structure.pools.StructurePoolElement>();
            if (depth != 7) targetPieces.addAll(targetPool.value().getShuffledTemplates(random));
            targetPieces.addAll(fallback.value().getShuffledTemplates(random));
            int placementPriority = sourceJigsaw.placementPriority();
            boolean attached = false;
            for (var targetElement : targetPieces) {
                if (targetElement == net.minecraft.world.level.levelgen.structure.pools.EmptyPoolElement.INSTANCE) break;
                for (var targetRotation : net.minecraft.world.level.block.Rotation.getShuffled(random)) {
                    var targetJigsaws = targetElement.getShuffledJigsawBlocks(
                        ctx.structureTemplateManager(), BlockPos.ZERO, targetRotation, random);
                    var hackBox = targetElement.getBoundingBox(ctx.structureTemplateManager(), BlockPos.ZERO, targetRotation);
                    int expandTo = 0; // doExpansionHack=false
                    for (var targetJigsaw : targetJigsaws) {
                        if (net.minecraft.world.level.block.JigsawBlock.canAttach(sourceJigsaw, targetJigsaw)) {
                            var targetJigsawLocalPos = targetJigsaw.info().pos();
                            var rawTargetBoxPos = targetJigsawPos.subtract(targetJigsawLocalPos);
                            var rawTargetBB = targetElement.getBoundingBox(ctx.structureTemplateManager(), rawTargetBoxPos, targetRotation);
                            int rawTargetY = rawTargetBB.minY();
                            var targetProjection = targetElement.getProjection();
                            boolean targetRigid = targetProjection == net.minecraft.world.level.levelgen.structure.pools.StructureTemplatePool.Projection.RIGID;
                            int targetJigsawLocalY = targetJigsawLocalPos.getY();
                            int deltaY = sourceJigsawLocalY - targetJigsawLocalY
                                + net.minecraft.world.level.block.JigsawBlock.getFrontFacing(sourceJigsawInfo.state()).getStepY();
                            int targetBoxY;
                            if (sourceRigid && targetRigid) {
                                targetBoxY = sourceBoxY + deltaY;
                            } else {
                                if (sourceJigsawBaseHeight == Integer.MIN_VALUE) {
                                    sourceJigsawBaseHeight = heightAccessor.getMinY(); // placeholder; rigid path only
                                }
                                targetBoxY = sourceJigsawBaseHeight - targetJigsawLocalY;
                            }
                            int yOffset = targetBoxY - rawTargetY;
                            var targetBB = rawTargetBB.moved(0, yOffset, 0);
                            var boxPos = rawTargetBoxPos.offset(0, yOffset, 0);
                            if (expandTo > 0) { /* disabled */ }
                            var freeShape = (net.minecraft.world.phys.shapes.VoxelShape) childrenFree[0];
                            var cand = net.minecraft.world.phys.shapes.Shapes.create(
                                net.minecraft.world.phys.AABB.of(targetBB).deflate(0.25));
                            if (!net.minecraft.world.phys.shapes.Shapes.joinIsNotEmpty(
                                    freeShape, cand, net.minecraft.world.phys.shapes.BooleanOp.ONLY_SECOND)) {
                                childrenFree[0] = net.minecraft.world.phys.shapes.Shapes.joinUnoptimized(
                                    freeShape,
                                    net.minecraft.world.phys.shapes.Shapes.create(net.minecraft.world.phys.AABB.of(targetBB)),
                                    net.minecraft.world.phys.shapes.BooleanOp.ONLY_FIRST);
                                int sourceGroundLevelDelta = sourcePiece.getGroundLevelDelta();
                                int targetGroundLevelDelta = targetRigid
                                    ? sourceGroundLevelDelta - deltaY
                                    : targetElement.getGroundLevelDelta();
                                var targetPiece = new net.minecraft.world.level.levelgen.structure.PoolElementStructurePiece(
                                    ctx.structureTemplateManager(), targetElement, boxPos,
                                    targetGroundLevelDelta, targetRotation, targetBB,
                                    JigsawStructure.DEFAULT_LIQUID_SETTINGS);
                                pieces.add(new Object[]{targetPiece, depth + 1});
                                System.out.printf(Locale.ROOT,
                                    "MINI-ACCEPT parent=%s child=%s rot=%s draws=%d%n",
                                    sourceElement.getClass().getSimpleName(), describe(targetElement),
                                    targetRotation, counting.draws);
                                attached = true;
                                break;
                            }
                        }
                    }
                    if (attached) break;
                }
                if (attached) break;
            }
        }
    }

    static String describe(net.minecraft.world.level.levelgen.structure.pools.StructurePoolElement el) {
        if (el instanceof net.minecraft.world.level.levelgen.structure.pools.SinglePoolElement spe) {
            try {
                var f = net.minecraft.world.level.levelgen.structure.pools.SinglePoolElement.class.getDeclaredField("template");
                f.setAccessible(true);
                Object o = f.get(spe);
                if (o instanceof com.mojang.datafixers.util.Either<?, ?> either && either.left().isPresent()) {
                    Object left = either.left().get();
                    return left instanceof net.minecraft.resources.ResourceKey<?> key
                        ? key.identifier().getPath() : left.toString();
                }
                return "INLINE";
            } catch (Exception e) { return "ERR"; }
        }
        if (el instanceof net.minecraft.world.level.levelgen.structure.pools.FeaturePoolElement) return "FEATURE";
        if (el instanceof net.minecraft.world.level.levelgen.structure.pools.ListPoolElement) return "LIST";
        if (el instanceof net.minecraft.world.level.levelgen.structure.pools.EmptyPoolElement) return "EMPTY";
        return el.getClass().getSimpleName();
    }
}
