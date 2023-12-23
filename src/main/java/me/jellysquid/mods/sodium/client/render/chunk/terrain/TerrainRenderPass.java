package me.jellysquid.mods.sodium.client.render.chunk.terrain;

import net.minecraft.client.render.RenderLayer;

public class TerrainRenderPass {
    private final RenderLayer layer;

    private final boolean useReverseOrder;
    private final boolean fragmentDiscard;

    public TerrainRenderPass(RenderLayer layer, boolean useReverseOrder, boolean allowFragmentDiscard) {
        this.layer = layer;

        this.useReverseOrder = useReverseOrder;
        this.fragmentDiscard = allowFragmentDiscard;
    }

    public boolean isReverseOrder() {
        return this.useReverseOrder;
    }

    public void render(ChunkMeshBufferBuilder buffer) {
        if (this.useReverseOrder) {
            buffer.render(this.layer, ChunkMeshBufferBuilder.DrawMode.BACK, ChunkMeshBufferBuilder.DrawMode.FRONT);
        } else {
            buffer.render(this.layer, ChunkMeshBufferBuilder.DrawMode.FRONT, ChunkMeshBufferBuilder.DrawMode.BACK);
        }
    }

    public boolean supportsFragmentDiscard() {
        return this.fragmentDiscard;
    }
}
