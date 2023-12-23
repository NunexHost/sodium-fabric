package me.jellysquid.mods.sodium.client.world.biome;

import me.jellysquid.mods.sodium.mixin.features.world.biome.FoliageColorsAccessor;
import me.jellysquid.mods.sodium.mixin.features.world.biome.GrassColorsAccessor;

public class BiomeColorMaps {
    private static final int WIDTH = 256;
    private static final int HEIGHT = 256;

    private static final int INVALID_INDEX = -1;
    private static final int DEFAULT_COLOR = 0xffff00ff;

    private static int[] grassColors = null;
    private static int[] foliageColors = null;

    public static int getGrassColor(int index) {
        if (index == INVALID_INDEX) {
            return DEFAULT_COLOR;
        }

        if (grassColors == null) {
            grassColors = GrassColorsAccessor.getColorMap().clone();
        }

        return grassColors[index];
    }

    public static int getFoliageColor(int index) {
        if (index == INVALID_INDEX) {
            return DEFAULT_COLOR;
        }

        if (foliageColors == null) {
            foliageColors = FoliageColorsAccessor.getColorMap().clone();
        }

        return foliageColors[index];
    }

    public static int getIndex(double temperature, double humidity) {
        humidity *= temperature;

        int x = (int) ((1.0D - temperature) * 255.0D);
        int y = (int) ((1.0D - humidity) * 255.0D);

        if (x < 0 || x >= WIDTH) {
            return INVALID_INDEX;
        }

        if (y < 0 || y >= HEIGHT) {
            return INVALID_INDEX;
        }

        return (y << 8) | x;
    }

    static {
        // Pre-calculate `getWidth()` and `getHeight()` to avoid repeated calls.
        getWidth();
        getHeight();
    }
}
