package me.jellysquid.mods.sodium.client.world.cloned;

import it.unimi.dsi.fastutil.longs.Long2ReferenceLinkedOpenHashMap;
import net.minecraft.util.math.ChunkSectionPos;
import net.minecraft.world.World;
import net.minecraft.world.chunk.ChunkSection;
import net.minecraft.world.chunk.WorldChunk;
import org.jetbrains.annotations.NotNull;
import org.jetbrains.annotations.Nullable;

import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.TimeUnit;

public class ClonedChunkSectionCache {
    private static final int MAX_CACHE_SIZE = 512; /* number of entries */
    private static final long MAX_CACHE_DURATION = TimeUnit.SECONDS.toNanos(5); /* number of nanoseconds */

    private final World world;

    private final ConcurrentHashMap<ChunkSectionPos, ClonedChunkSection> positionToEntry = new ConcurrentHashMap<>();

    private long time; // updated once per frame to be the elapsed time since application start

    public ClonedChunkSectionCache(World world) {
        this.world = world;
        this.time = getMonotonicTimeSource();
    }

    public void cleanup() {
        this.time = getMonotonicTimeSource();

        // Remove expired entries
        this.positionToEntry.entrySet().removeIf(entry -> this.time > (entry.getValue().getLastUsedTimestamp() + MAX_CACHE_DURATION));
    }

    @Nullable
    public ClonedChunkSection acquire(int x, int y, int z) {
        ChunkSectionPos chunkSectionPos = ChunkSectionPos.from(x, y, z);
        ClonedChunkSection section = this.positionToEntry.get(chunkSectionPos);

        if (section == null) {
            section = this.clone(x, y, z);
            this.positionToEntry.put(chunkSectionPos, section);
        }

        section.setLastUsedTimestamp(this.time);

        return section;
    }

    @NotNull
    private ClonedChunkSection clone(int x, int y, int z) {
        WorldChunk chunk = this.world.getChunk(x, z);

        if (chunk == null) {
            throw new RuntimeException("Chunk is not loaded at: " + ChunkSectionPos.asLong(x, y, z));
        }

        @Nullable ChunkSection section = null;

        if (!this.world.isOutOfHeightLimit(ChunkSectionPos.getBlockCoord(y))) {
            section = chunk.getSectionArray()[this.world.sectionCoordToIndex(y)];
        }

        return new ClonedChunkSection(this.world, chunk, section, chunkSectionPos);
    }

    public void invalidate(int x, int y, int z) {
        this.positionToEntry.remove(ChunkSectionPos.from(x, y, z));
    }

    private static long getMonotonicTimeSource() {
        // Should be monotonic in JDK 17 on sane platforms...
        return System.nanoTime();
    }
}
