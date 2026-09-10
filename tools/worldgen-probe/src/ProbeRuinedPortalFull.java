import java.nio.file.Path;
import net.minecraft.core.BlockPos;
import net.minecraft.core.QuartPos;
import java.util.ArrayList;
import java.util.List;
import java.util.Locale;
import java.util.Optional;
import java.lang.reflect.Proxy;
import net.minecraft.SharedConstants;
import net.minecraft.server.Bootstrap;
import net.minecraft.core.Holder;
import net.minecraft.core.HolderGetter;
import net.minecraft.core.HolderLookup;
import net.minecraft.core.RegistryAccess;
import net.minecraft.core.registries.Registries;
import net.minecraft.data.registries.VanillaRegistries;
import net.minecraft.resources.Identifier;
import net.minecraft.resources.ResourceKey;
import net.minecraft.server.packs.PackType;
import net.minecraft.server.packs.VanillaPackResourcesBuilder;
import net.minecraft.server.packs.PackLocationInfo;
import net.minecraft.server.packs.repository.PackSource;
import net.minecraft.server.packs.resources.MultiPackResourceManager;
import net.minecraft.util.datafix.DataFixers;
import net.minecraft.world.level.biome.Biome;
import net.minecraft.world.level.biome.MultiNoiseBiomeSource;
import net.minecraft.world.level.biome.MultiNoiseBiomeSourceParameterList;
import net.minecraft.world.level.biome.MultiNoiseBiomeSourceParameterLists;
import net.minecraft.world.level.LevelHeightAccessor;
import net.minecraft.world.level.chunk.ChunkGeneratorStructureState;
import net.minecraft.world.level.levelgen.NoiseBasedChunkGenerator;
import net.minecraft.world.level.levelgen.NoiseGeneratorSettings;
import net.minecraft.world.level.levelgen.RandomState;
import net.minecraft.world.level.levelgen.WorldgenRandom;
import net.minecraft.world.level.levelgen.structure.BoundingBox;
import net.minecraft.world.level.levelgen.structure.Structure;
import net.minecraft.world.level.levelgen.structure.StructureSet;
import net.minecraft.world.level.levelgen.structure.StructureStart;
import net.minecraft.world.level.levelgen.structure.templatesystem.StructureTemplate;
import net.minecraft.world.level.levelgen.structure.templatesystem.StructureTemplateManager;
import net.minecraft.network.chat.Component;

/**
 * FULL ruined-portal oracle: real template loading, real biome tags,
 * ChunkGenerator.createStructures retry loop replica. Prints every attempt.
 */
public class ProbeRuinedPortalFull {
    static final LevelHeightAccessor HEIGHT = new LevelHeightAccessor() {
        @Override public int getHeight() { return 384; }
        @Override public int getMinY() { return -64; }
    };

    public static void main(String[] args) throws Exception {
        long seed = args.length > 0 ? Long.parseLong(args[0]) : 424242L;
        int cx = args.length > 1 ? Integer.parseInt(args[1]) : 8;
        int cz = args.length > 2 ? Integer.parseInt(args[2]) : 2;
        SharedConstants.tryDetectVersion();
        Bootstrap.bootStrap();
        ProbeCoalOre.bindBlockTags();
        var lookup = VanillaRegistries.createLookup();
        RegistryAccess regAccess = (RegistryAccess) ProbeCoalOre.regAccessStub((HolderLookup.Provider) lookup);

        var noises = lookup.lookupOrThrow(Registries.NOISE);
        var settingsHolder = lookup.lookupOrThrow(Registries.NOISE_SETTINGS)
                .getOrThrow(NoiseGeneratorSettings.OVERWORLD);
        RandomState rs = RandomState.create(settingsHolder.value(), noises, seed);

        var plReg = lookup.lookupOrThrow(Registries.MULTI_NOISE_BIOME_SOURCE_PARAMETER_LIST);
        var plKey = ResourceKey.create(Registries.MULTI_NOISE_BIOME_SOURCE_PARAMETER_LIST,
                Identifier.parse("minecraft:overworld"));
        var biomeSource = MultiNoiseBiomeSource.createFromPreset(plReg.getOrThrow(plKey));
        NoiseBasedChunkGenerator generator = new NoiseBasedChunkGenerator(biomeSource, settingsHolder);

        // Vanilla jar resources as a SERVER_DATA pack so templates resolve.
        var pack = new VanillaPackResourcesBuilder()
                .setMetadata(net.minecraft.server.packs.resources.ResourceMetadata.EMPTY)
                .exposeNamespace("minecraft")
                .pushJarResources()
                .build(new PackLocationInfo("vanilla", Component.literal("vanilla"), PackSource.BUILT_IN, Optional.empty()));
        var rm = new MultiPackResourceManager(PackType.SERVER_DATA, List.of(pack));

        Path storagePath = Path.of("/tmp/opencode/rp-level");
        var storageSource = net.minecraft.world.level.storage.LevelStorageSource.createDefault(storagePath);
        var access = storageSource.createAccess("probe");
        var templates = new StructureTemplateManager(rm, access, DataFixers.getDataFixer(),
                lookup.lookupOrThrow(Registries.BLOCK));

        String[] ids = {"minecraft:ruined_portal", "minecraft:ruined_portal_desert",
            "minecraft:ruined_portal_jungle", "minecraft:ruined_portal_swamp",
            "minecraft:ruined_portal_mountain", "minecraft:ruined_portal_ocean",
            "minecraft:ruined_portal_nether"};
        List<StructureSet.StructureSelectionEntry> options0 = new ArrayList<>();
        for (String id : ids) {
            Holder<Structure> h = lookup.lookupOrThrow(Registries.STRUCTURE)
                    .getOrThrow(ResourceKey.create(Registries.STRUCTURE, Identifier.parse(id)));
            options0.add(new StructureSet.StructureSelectionEntry(h, 1));
        }

        System.out.printf(Locale.ROOT, "PROBE-FULL seed=%d anchor=(%d,%d)%n", seed, cx, cz);

        // ---- ChunkGenerator.createStructures retry-loop replica (:509-551)
        List<StructureSet.StructureSelectionEntry> options = new ArrayList<>(options0);
        WorldgenRandom drv = new WorldgenRandom(new net.minecraft.world.level.levelgen.LegacyRandomSource(0L));
        drv.setLargeFeatureSeed(seed, cx, cz);
        int total = options.size();
        int attempt = 0;
        while (!options.isEmpty()) {
            int choice = drv.nextInt(total);
            int index = 0;
            for (int i = 0; i < options.size(); i++) {
                choice -= options.get(i).weight();
                if (choice < 0) break;
                index++;
            }
            StructureSet.StructureSelectionEntry selected = options.get(index);
            Structure s = selected.structure().value();
            String sid = selected.structure().unwrapKey().map(k -> k.identifier().toString()).orElse("?");
            attempt++;
            java.util.function.Predicate<Holder<Biome>> pred = biomePredFor(sid);
            Structure.GenerationContext ctx = new Structure.GenerationContext(
                    regAccess, generator, biomeSource, rs, templates, seed,
                    new net.minecraft.world.level.ChunkPos(cx, cz), HEIGHT,
                    pred);
            Optional<Structure.GenerationStub> stubOpt;
            try {
                stubOpt = s.findValidGenerationPoint(ctx);
            } catch (Throwable t) {
                System.out.printf(Locale.ROOT, "ATTEMPT %d %s THREW %s%n", attempt, sid, t);
                options.remove(index);
                total -= selected.weight();
                continue;
            }
            if (stubOpt.isPresent()) {
                var stub = stubOpt.get();
                BlockPos p = stub.position();
                StringBuilder bbStr = new StringBuilder();
                try {
                    var pieces = stub.getPiecesBuilder().build().pieces();
                    for (var piece : pieces) {
                        BoundingBox b = piece.getBoundingBox();
                        bbStr.append(String.format(Locale.ROOT,
                                " bbox[x%d..%d y%d..%d z%d..%d]", b.minX(), b.maxX(), b.minY(), b.maxY(), b.minZ(), b.maxZ()));
                    }
                } catch (Throwable t) {
                    bbStr.append(" <piece-build-failed ").append(t).append('>');
                }
                System.out.printf(Locale.ROOT,
                        "ATTEMPT %d %s ACCEPT origin=(%d,%d,%d)%s%n",
                        attempt, sid, p.getX(), p.getY(), p.getZ(), bbStr);
                return;
            } else {
                System.out.printf(Locale.ROOT, "ATTEMPT %d %s reject(stub-biome)%n", attempt, sid);
            }
            options.remove(index);
            total -= selected.weight();
        }
        System.out.println("NO-START");
        access.close();
    }

    static final java.util.Set<String> STD = new java.util.HashSet<>(java.util.List.of(
        "minecraft:beach","minecraft:snowy_beach","minecraft:river","minecraft:frozen_river",
        "minecraft:taiga","minecraft:snowy_taiga","minecraft:old_growth_pine_taiga",
        "minecraft:old_growth_spruce_taiga","minecraft:forest","minecraft:flower_forest",
        "minecraft:birch_forest","minecraft:old_growth_birch_forest","minecraft:dark_forest",
        "minecraft:pale_garden","minecraft:grove","minecraft:mushroom_fields","minecraft:ice_spikes",
        "minecraft:dripstone_caves","minecraft:lush_caves","minecraft:sulfur_caves","minecraft:savanna",
        "minecraft:savanna_plateau","minecraft:windswept_savanna","minecraft:snowy_plains","minecraft:plains",
        "minecraft:sunflower_plains"));
    static boolean allowed(String sid, String b) {
        return switch (sid) {
            case "minecraft:ruined_portal" -> STD.contains(b);
            case "minecraft:ruined_portal_desert" -> b.equals("minecraft:desert");
            case "minecraft:ruined_portal_jungle" -> b.equals("minecraft:jungle") || b.equals("minecraft:sparse_jungle") || b.equals("minecraft:bamboo_jungle");
            case "minecraft:ruined_portal_swamp" -> b.equals("minecraft:swamp") || b.equals("minecraft:mangrove_swamp");
            case "minecraft:ruined_portal_mountain" -> java.util.Set.of("minecraft:badlands","minecraft:eroded_badlands","minecraft:wooded_badlands",
                "minecraft:windswept_hills","minecraft:windswept_forest","minecraft:windswept_gravelly_hills",
                "minecraft:savanna_plateau","minecraft:windswept_savanna","minecraft:stony_shore","minecraft:meadow",
                "minecraft:frozen_peaks","minecraft:jagged_peaks","minecraft:stony_peaks","minecraft:snowy_slopes",
                "minecraft:cherry_grove").contains(b);
            case "minecraft:ruined_portal_ocean" -> b.contains("ocean");
            default -> false;
        };
    }
    static java.util.function.Predicate<Holder<Biome>> biomePredFor(String sid) {
        return h -> {
            String n = h.unwrapKey().map(k -> k.identifier().toString()).orElse("");
            if (n.isEmpty() && h.value() != null) { /* fallback */ }
            return allowed(sid, n);
        };
    }
}
