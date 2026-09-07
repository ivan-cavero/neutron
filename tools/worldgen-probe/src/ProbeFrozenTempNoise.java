import net.minecraft.SharedConstants;
import net.minecraft.server.Bootstrap;
import net.minecraft.world.level.biome.Biome;
import java.lang.reflect.Field;
import java.io.BufferedReader;
import java.io.InputStreamReader;

public class ProbeFrozenTempNoise {
    public static void main(String[] args) throws Exception {
        SharedConstants.tryDetectVersion();
        Bootstrap.bootStrap();
        Field f = Biome.class.getDeclaredField("FROZEN_TEMPERATURE_NOISE");
        f.setAccessible(true);
        Object noise = f.get(null);
        java.lang.reflect.Method m = noise.getClass().getMethod("getValue", double.class, double.class, boolean.class);
        BufferedReader in = new BufferedReader(new InputStreamReader(System.in));
        String line;
        while ((line = in.readLine()) != null) {
            line = line.trim();
            if (line.isEmpty()) continue;
            String[] p = line.split("\\s+");
            double x = Double.parseDouble(p[0]);
            double z = Double.parseDouble(p[1]);
            System.out.printf("FROZEN %.9f%n", (Double) m.invoke(noise, x, z, false));
        }
    }
}
