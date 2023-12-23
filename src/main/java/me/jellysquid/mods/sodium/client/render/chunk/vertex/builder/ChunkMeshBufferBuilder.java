package me.jellysquid.mods.sodium.client.render.chunk.vertex.builder;

import me.jellysquid.mods.sodium.client.render.chunk.terrain.material.Material;
import me.jellysquid.mods.sodium.client.render.chunk.vertex.format.ChunkVertexEncoder;
import me.jellysquid.mods.sodium.client.render.chunk.vertex.format.ChunkVertexType;
import org.lwjgl.system.MemoryUtil;

import java.nio.ByteBuffer;

public class ChunkMeshBufferBuilder {
    private final ChunkVertexEncoder encoder;
    private final int stride;

    private final int initialCapacity;

    private ByteBuffer buffer;
    private int count;
    private int capacity;
    private int sectionIndex;

    public ChunkMeshBufferBuilder(ChunkVertexType vertexType, int initialCapacity) {
        this.encoder = vertexType.getEncoder();
        this.stride = vertexType.getVertexFormat().getStride();

        this.buffer = MemoryUtil.memAllocDirect(initialCapacity * this.stride);

        this.capacity = initialCapacity;
        this.initialCapacity = initialCapacity;
    }

    public void push(ChunkVertexEncoder.Vertex[] vertices, Material material) {
        var vertexStart = this.count;
        var vertexCount = vertices.length;

        if (this.count + vertexCount > this.capacity) {
            this.grow(vertexCount);
        }

        long ptr = MemoryUtil.memAddress(this.buffer, this.count * this.stride);

        for (ChunkVertexEncoder.Vertex vertex : vertices) {
            ptr = this.encoder.write(ptr, material, vertex, this.sectionIndex);
        }

        this.count += vertexCount;
    }

    private void grow(int len) {
        // The new capacity will be at least as large as the write it needs to service
        int cap = Math.max(this.capacity * 2, this.capacity + len);

        // Use a direct buffer to avoid unnecessary copies
        ByteBuffer newBuffer = MemoryUtil.memAllocDirect(cap * this.stride);

        // Copy the existing data to the new buffer
        MemoryUtil.memCopy(this.buffer, 0, newBuffer, 0, this.count * this.stride);

        // Update the references
        this.buffer = newBuffer;
        this.capacity = cap;
    }

    public void start(int sectionIndex) {
        this.count = 0;
        this.sectionIndex = sectionIndex;
    }

    public void destroy() {
        if (this.buffer != null) {
            MemoryUtil.memFree(this.buffer);
        }

        this.buffer = null;
    }

    public boolean isEmpty() {
        return this.count == 0;
    }

    public ByteBuffer slice() {
        if (this.isEmpty()) {
            throw new IllegalStateException("No vertex data in buffer");
        }

        return this.buffer.slice(0, this.count * this.stride);
    }

    public int count() {
        return this.count;
    }

    // Additional changes:

    /**
     * Returns the current capacity of the buffer, in bytes.
     */
    public int getCapacity() {
        return this.capacity * this.stride;
    }

    /**
     * Returns the amount of unused space remaining in the buffer, in bytes.
     */
    public int getRemainingCapacity() {
        return this.capacity - this.count * this.stride;
    }
}
