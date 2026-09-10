import net.minecraft.SharedConstants;
import net.minecraft.core.registries.Registries;
import net.minecraft.data.registries.VanillaRegistries;
import net.minecraft.server.Bootstrap;
public class ProbeRuinedPortalKeys {
    public static void main(String[] a) {
        SharedConstants.tryDetectVersion();
        Bootstrap.bootStrap();
        var lookup = VanillaRegistries.createLookup();
        var structures = lookup.lookupOrThrow(Registries.STRUCTURE);
        int n = 0;
        var it = structures.listElements().toList();
        System.out.println("total structures: " + it.size());
        for (var h : structures.listElements().toList()) {
            String id = h.key().identifier().toString();
            String step = h.value().step().getName();
            System.out.println(step + " " + id);
        }
    }
}
