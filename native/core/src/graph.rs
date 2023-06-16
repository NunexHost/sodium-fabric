use rustc_hash::FxHashMap as HashMap;

use std::cell::{RefCell, RefMut};
use std::collections::VecDeque;
use std::fmt::Debug;
use std::marker::PhantomData;
use std::mem::MaybeUninit;
use std::ptr::NonNull;
use std::simd::{i32x4, SimdPartialOrd, ToBitMask};
use std::vec::Vec;
use std::{ops::*, ptr};

use std::ops;

use crate::collections::ArrayDeque;
use crate::ffi::CInlineVec;
use crate::ffi::CVec;
use crate::frustum::{BoundingBox, Frustum};
use crate::math::*;

const SECTIONS_IN_REGION: usize = 8 * 4 * 8;

#[derive(Clone, Copy)]
pub struct LocalNodeIndex(u8);

impl LocalNodeIndex {
    const X_BITS: u8 = 0b111;
    const Y_BITS: u8 = 0b11;
    const Z_BITS: u8 = 0b111;

    const X_OFFSET: usize = 5;
    const Y_OFFSET: usize = 0;
    const Z_OFFSET: usize = 2;

    const X_MASK: u8 = Self::X_BITS << Self::X_OFFSET;
    const Y_MASK: u8 = Self::Y_BITS << Self::Y_OFFSET;
    const Z_MASK: u8 = Self::Z_BITS << Self::Z_OFFSET;

    #[inline(always)]
    fn from_global(position: IVec3) -> LocalNodeIndex {
        let packed = ((position.x() as u8 & Self::X_BITS) << Self::X_OFFSET)
            | ((position.y() as u8 & Self::Y_BITS) << Self::Y_OFFSET)
            | ((position.z() as u8 & Self::Z_BITS) << Self::Z_OFFSET);
        LocalNodeIndex(packed)
    }

    pub fn inc_x(self) -> (LocalNodeIndex, bool) {
        let result = (self.0 & !Self::X_MASK) | ((self.0 + (1 << Self::X_OFFSET)) & Self::X_MASK);
        (LocalNodeIndex(result), result < self.0)
    }

    pub fn dec_x(self) -> (LocalNodeIndex, bool) {
        let result = (self.0 & !Self::X_MASK) | ((self.0 - (1 << Self::X_OFFSET)) & Self::X_MASK);
        (LocalNodeIndex(result), result > self.0)
    }

    pub fn inc_y(self) -> (LocalNodeIndex, bool) {
        let result = (self.0 & !Self::Y_MASK) | ((self.0 + (1 << Self::Y_OFFSET)) & Self::Y_MASK);
        (LocalNodeIndex(result), result < self.0)
    }

    pub fn dec_y(self) -> (LocalNodeIndex, bool) {
        let result = (self.0 & !Self::Y_MASK) | ((self.0 - (1 << Self::Y_OFFSET)) & Self::Y_MASK);
        (LocalNodeIndex(result), result > self.0)
    }

    pub fn inc_z(self) -> (LocalNodeIndex, bool) {
        let result = (self.0 & !Self::Z_MASK) | ((self.0 + (1 << Self::Z_OFFSET)) & Self::Z_MASK);
        (LocalNodeIndex(result), result < self.0)
    }

    pub fn dec_z(self) -> (LocalNodeIndex, bool) {
        let result = (self.0 & !Self::Z_MASK) | ((self.0 - (1 << Self::Z_OFFSET)) & Self::Z_MASK);
        (LocalNodeIndex(result), result > self.0)
    }

    pub fn x(&self) -> i32 {
        ((self.0 >> Self::X_OFFSET) & Self::X_BITS) as i32
    }

    pub fn y(&self) -> i32 {
        ((self.0 >> Self::Y_OFFSET) & Self::Y_BITS) as i32
    }

    pub fn z(&self) -> i32 {
        ((self.0 >> Self::Z_OFFSET) & Self::Z_BITS) as i32
    }

    fn as_array_offset(&self) -> usize {
        self.0 as usize
    }

    fn as_global_coord(&self, region_coord: IVec3) -> IVec3 {
        region_coord
            .shl(IVec3::new(3, 2, 3))
            .add(IVec3::new(self.x(), self.y(), self.z()))
    }
}

#[derive(Clone, Copy)]
pub struct PackedChunkCoord(u64);

impl PackedChunkCoord {
    const TOTAL_SIZE: usize = u64::BITS as usize;

    const X_MASK: u64 = 0x3FFFFF;
    const Y_MASK: u64 = 0x0FFFFF;
    const Z_MASK: u64 = 0x3FFFFF;

    const X_SIZE: usize = 22;
    const Y_SIZE: usize = 20;
    const Z_SIZE: usize = 22;

    const X_OFFSET: usize = 42;
    const Y_OFFSET: usize = 0;
    const Z_OFFSET: usize = 20;

    pub fn from(coord: IVec3) -> Self {
        let mut packed: u64 = 0;
        packed |= (coord.x() as u64 & Self::X_MASK) << Self::X_OFFSET;
        packed |= (coord.y() as u64 & Self::Y_MASK) << Self::Y_OFFSET;
        packed |= (coord.z() as u64 & Self::Z_MASK) << Self::Z_OFFSET;

        PackedChunkCoord(packed)
    }

    pub fn x(&self) -> i32 {
        ((self.0 << (Self::TOTAL_SIZE - Self::X_SIZE - Self::X_OFFSET))
            >> (Self::TOTAL_SIZE - Self::X_SIZE)) as i32
    }

    pub fn y(&self) -> i32 {
        ((self.0 << (Self::TOTAL_SIZE - Self::Y_SIZE - Self::Y_OFFSET))
            >> (Self::TOTAL_SIZE - Self::Y_SIZE)) as i32
    }

    pub fn z(&self) -> i32 {
        ((self.0 << (Self::TOTAL_SIZE - Self::Z_SIZE - Self::Z_OFFSET))
            >> (Self::TOTAL_SIZE - Self::Z_SIZE)) as i32
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum GraphDirection {
    NegX,
    NegY,
    NegZ,
    PosX,
    PosY,
    PosZ,
}

impl GraphDirection {
    pub const fn as_vector(&self) -> IVec3 {
        match *self {
            GraphDirection::NegX => IVec3::new(-1, 0, 0),
            GraphDirection::NegY => IVec3::new(0, -1, 0),
            GraphDirection::NegZ => IVec3::new(0, 0, -1),
            GraphDirection::PosX => IVec3::new(1, 0, 0),
            GraphDirection::PosY => IVec3::new(0, 1, 0),
            GraphDirection::PosZ => IVec3::new(0, 0, 1),
        }
    }

    pub const fn ordered() -> &'static [GraphDirection; 6] {
        const ORDERED: [GraphDirection; 6] = [
            GraphDirection::NegX,
            GraphDirection::NegY,
            GraphDirection::NegZ,
            GraphDirection::PosX,
            GraphDirection::PosY,
            GraphDirection::PosZ,
        ];
        &ORDERED
    }

    pub const fn opposite(&self) -> GraphDirection {
        match *self {
            GraphDirection::NegX => GraphDirection::PosX,
            GraphDirection::NegY => GraphDirection::PosY,
            GraphDirection::NegZ => GraphDirection::PosZ,
            GraphDirection::PosX => GraphDirection::NegX,
            GraphDirection::PosY => GraphDirection::NegY,
            GraphDirection::PosZ => GraphDirection::NegZ,
        }
    }
}

#[derive(Clone, Copy)]
pub struct GraphDirectionSet(u8);

impl GraphDirectionSet {
    pub fn from(packed: u8) -> Self {
        GraphDirectionSet(packed)
    }

    pub fn none() -> GraphDirectionSet {
        GraphDirectionSet(0)
    }

    pub fn all() -> GraphDirectionSet {
        let mut set = GraphDirectionSet::none();

        for dir in GraphDirection::ordered() {
            set.add(*dir);
        }

        set
    }

    pub fn single(direction: GraphDirection) -> GraphDirectionSet {
        let mut set = GraphDirectionSet::none();
        set.add(direction);
        set
    }

    pub fn add(&mut self, dir: GraphDirection) {
        self.0 |= 1 << dir as usize;
    }

    pub fn add_all(&mut self, set: GraphDirectionSet) {
        self.0 |= set.0;
    }

    pub fn contains(&self, dir: GraphDirection) -> bool {
        (self.0 & (1 << dir as usize)) != 0
    }

    fn is_empty(&self) -> bool {
        self.0 == 0
    }
}

impl Default for GraphDirectionSet {
    fn default() -> Self {
        GraphDirectionSet::none()
    }
}

impl ops::BitAnd for GraphDirectionSet {
    type Output = GraphDirectionSet;

    fn bitand(self, rhs: Self) -> Self::Output {
        GraphDirectionSet(self.0 & rhs.0)
    }
}

#[derive(Default, Clone, Copy)]
pub struct VisibilityData {
    data: [u8; 6],
}

impl VisibilityData {
    pub fn from_u64(packed: u64) -> Self {
        VisibilityData {
            data: VisibilityData::unpack(packed),
        }
    }

    fn get_outgoing_directions(&self, incoming: GraphDirectionSet) -> GraphDirectionSet {
        let packed = VisibilityData::pack(&self.data);

        let mut outgoing = (((0b_0000001_0000001_0000001_0000001_0000001_0000001u64 * incoming.0 as u64) & 0x010101010101_u64) * 0xFF) // turn bitmask into lane wise mask
            & packed; // apply visibility to incoming
        outgoing |= outgoing >> 32; // fold top 32 bits onto bottom 32 bits
        outgoing |= outgoing >> 16; // fold top 16 bits onto bottom 16 bits
        outgoing |= outgoing >> 8; // fold top 8 bits onto bottom 8 bits

        GraphDirectionSet(outgoing as u8)
    }

    fn unpack(packed: u64) -> [u8; 6] {
        let mut data = [0u8; 6];

        unsafe {
            data.copy_from_slice(&packed.to_ne_bytes()[0..6]);
        }

        data
    }

    fn pack(data: &[u8; 6]) -> u64 {
        let mut packed = [0u8; 8];
        packed[0..6].copy_from_slice(data);

        unsafe { u64::from_ne_bytes(packed) }
    }
}

#[repr(C)]
pub struct RegionDrawBatch {
    region_coord: (i32, i32, i32),
    sections: CInlineVec<LocalNodeIndex, SECTIONS_IN_REGION>,
}

impl RegionDrawBatch {
    pub fn new(region_coord: IVec3) -> Self {
        RegionDrawBatch {
            region_coord: region_coord.into(),
            sections: CInlineVec::new(),
        }
    }

    fn is_empty(&self) -> bool {
        self.sections.is_empty()
    }
}

#[derive(Default, Clone, Copy)]
pub struct Node {
    pub connections: VisibilityData,
    pub flags: u8,
}

impl Node {
    pub fn new(connections: VisibilityData, flags: u8) -> Self {
        Node { connections, flags }
    }
}

struct RegionSearchState {
    incoming: [GraphDirectionSet; SECTIONS_IN_REGION],
    queue: ArrayDeque<LocalNodeIndex, { SECTIONS_IN_REGION + 1 }>,

    enqueued: bool,
}

impl RegionSearchState {
    pub fn enqueue(&mut self, index: LocalNodeIndex, directions: GraphDirectionSet) {
        let incoming = &mut self.incoming[index.as_array_offset()];
        let should_enqueue = incoming.is_empty();

        incoming.add_all(directions);

        unsafe {
            self.queue
                .push_conditionally_unchecked(index, should_enqueue);
        }
    }

    fn reset(&mut self) {
        self.queue.reset();
        self.incoming.fill(GraphDirectionSet::none());

        self.enqueued = false;
    }
}

impl Default for RegionSearchState {
    fn default() -> Self {
        Self {
            queue: Default::default(),
            incoming: [GraphDirectionSet::default(); SECTIONS_IN_REGION],
            enqueued: false,
        }
    }
}

struct Region {
    nodes: [Node; SECTIONS_IN_REGION],
    search_state: RegionSearchState,

    loaded_count: usize,
}

impl Region {
    fn new() -> Region {
        Region {
            nodes: [Node::default(); SECTIONS_IN_REGION],
            search_state: RegionSearchState::default(),

            loaded_count: 0,
        }
    }

    fn set_chunk(&mut self, coord: IVec3, node: Node) {
        let local_index = LocalNodeIndex::from_global(coord);
        self.nodes[local_index.as_array_offset()] = node;
    }

    fn remove_chunk(&mut self, coord: IVec3) {
        let local_index = LocalNodeIndex::from_global(coord);
        self.nodes[local_index.as_array_offset()] = Node::default();
    }

    fn get_chunk(&self, coord: IVec3) -> &Node {
        let local_index = LocalNodeIndex::from_global(coord);
        &self.nodes[local_index.as_array_offset()]
    }
}

pub struct Graph {
    regions: HashMap<IVec3, Region>,
}

impl Graph {
    pub fn new() -> Self {
        Graph {
            regions: HashMap::default(),
        }
    }

    pub fn search(&mut self, frustum: &Frustum, view_distance: i32) -> CVec<RegionDrawBatch> {
        let mut region_iteration_queue: VecDeque<IVec3> = VecDeque::new();

        let origin_node_coord = position_to_chunk_coord(*frustum.position());

        if let Some(node) = self.get_node(origin_node_coord) {
            let region_coord = chunk_coord_to_region_coord(origin_node_coord);

            let mut region = self.regions.get_mut(&region_coord).unwrap();
            region.search_state.enqueue(
                LocalNodeIndex::from_global(origin_node_coord),
                GraphDirectionSet::all(),
            );
            region.search_state.enqueued = true;

            region_iteration_queue.push_back(region_coord);
        }

        let mut sorted_batches: Vec<RegionDrawBatch> = Vec::new();

        while let Some(region_coord) = region_iteration_queue.pop_front() {
            let mut search_ctx = SearchContext::create(&mut self.regions, region_coord);
            let mut batch: RegionDrawBatch = RegionDrawBatch::new(region_coord);

            while let Some(node_idx) = search_ctx.origin().search_state.queue.pop() {
                let node_coord = node_idx.as_global_coord(region_coord);

                let node = search_ctx.origin().nodes[node_idx.as_array_offset()];
                let node_incoming =
                    search_ctx.origin().search_state.incoming[node_idx.as_array_offset()];

                if !chunk_inside_view_distance(node_coord, origin_node_coord, view_distance)
                    || !chunk_inside_frustum(node_coord, frustum)
                {
                    continue;
                }

                if (node.flags & (1 << 1)) != 0 {
                    batch.sections.push(node_idx);
                }

                let valid_directions = get_valid_directions(origin_node_coord, node_coord);
                let allowed_directions =
                    node.connections.get_outgoing_directions(node_incoming) & valid_directions;

                Self::enqueue_all_neighbors(&mut search_ctx, allowed_directions, node_idx);
            }

            if !batch.is_empty() {
                sorted_batches.push(batch);
            }

            for direction in GraphDirection::ordered() {
                let adjacent_region_coord: IVec3 = region_coord + direction.as_vector();

                if let Some(region) = &mut search_ctx.adjacent(*direction, true) {
                    if region.search_state.queue.is_empty() || region.search_state.enqueued {
                        continue;
                    }

                    region.search_state.enqueued = true;
                    region_iteration_queue.push_back(adjacent_region_coord);
                }
            }

            search_ctx.origin().search_state.reset();
        }

        CVec::from_boxed_slice(sorted_batches.into_boxed_slice())
    }

    fn enqueue_all_neighbors(
        context: &mut SearchContext,
        directions: GraphDirectionSet,
        origin: LocalNodeIndex,
    ) {
        for direction in GraphDirection::ordered() {
            if directions.contains(*direction) {
                let (neighbor, wrapped) = match direction {
                    GraphDirection::NegX => origin.dec_x(),
                    GraphDirection::NegY => origin.dec_y(),
                    GraphDirection::NegZ => origin.dec_z(),
                    GraphDirection::PosX => origin.inc_x(),
                    GraphDirection::PosY => origin.inc_y(),
                    GraphDirection::PosZ => origin.inc_z(),
                };

                if let Some(neighbor_region) = context.adjacent(*direction, wrapped) {
                    neighbor_region
                        .search_state
                        .enqueue(neighbor, GraphDirectionSet::single(direction.opposite()));
                }
            }
        }
    }

    pub fn add_chunk(&mut self, chunk_coord: IVec3) {
        let mut region = self
            .regions
            .entry(chunk_coord_to_region_coord(chunk_coord))
            .or_insert_with(|| Region::new());

        region.set_chunk(chunk_coord, Node::default());
    }

    pub fn update_chunk(&mut self, chunk_coord: IVec3, node: Node) {
        if let Some(region) = self
            .regions
            .get_mut(&chunk_coord_to_region_coord(chunk_coord))
        {
            region.set_chunk(chunk_coord, node);
        }
    }

    pub fn remove_chunk(&mut self, chunk_coord: IVec3) {
        if let Some(region) = self
            .regions
            .get_mut(&chunk_coord_to_region_coord(chunk_coord))
        {
            region.remove_chunk(chunk_coord);
        }
    }

    fn get_node(&self, chunk_coord: IVec3) -> Option<Node> {
        self.regions
            .get(&chunk_coord_to_region_coord(chunk_coord))
            .map(|region| *region.get_chunk(chunk_coord))
    }
}

#[repr(C)] // SAFETY: This ensures the layout of our struct will not change
pub struct SearchContext<'a> {
    adjacent: [*mut Region; 6],
    origin: NonNull<Region>,

    reference: PhantomData<&'a mut HashMap<IVec3, Region>>,
}

impl<'a> SearchContext<'a> {
    fn adjacent(&mut self, direction: GraphDirection, wrapped: bool) -> Option<&'a mut Region> {
        unsafe {
            // SAFETY: The C layout ensures the field SearchContext.origin will always be the N+6 element
            let offset = if wrapped { direction as usize } else { 6 };
            let ptr = self.adjacent.as_ptr().add(offset);

            // SAFETY: Pointer is always valid, if non-null
            NonNull::new(*ptr).map(|mut ptr| ptr.as_mut())
        }
    }

    fn origin(&mut self) -> &'a mut Region {
        unsafe {
            // SAFETY: Not possible to take more than one reference at a time
            self.origin.as_mut()
        }
    }

    fn create(regions: &'a mut HashMap<IVec3, Region>, origin_coord: IVec3) -> SearchContext<'a> {
        SearchContext {
            adjacent: GraphDirection::ordered()
                .map(|direction| origin_coord + direction.as_vector())
                .map(|position| Self::get_ptr(regions, &position)),

            origin: NonNull::new(Self::get_ptr(regions, &origin_coord))
                .expect("Origin region does not exist"),

            reference: PhantomData,
        }
    }

    fn get_ptr(regions: &mut HashMap<IVec3, Region>, position: &IVec3) -> *mut Region {
        regions
            .get_mut(position)
            .map_or(ptr::null_mut(), |cell| cell as *mut Region)
    }
}

fn chunk_inside_view_distance(position: IVec3, center: IVec3, view_distance: i32) -> bool {
    let distance: IVec3 = (position - center).abs();
    distance.less_than(IVec3::splat(view_distance))
}

fn get_valid_directions(center: IVec3, position: IVec3) -> GraphDirectionSet {
    let position: i32x4 = position.into();
    let center: i32x4 = center.into();

    let negative = position.simd_le(center);
    let positive = position.simd_ge(center);

    GraphDirectionSet::from(
        (negative.to_bitmask() & 0b111) | ((positive.to_bitmask() & 0b111) << 3),
    )
}

fn chunk_inside_frustum(position: IVec3, frustum: &Frustum) -> bool {
    frustum.test_bounding_box(&get_chunk_bounding_box(position))
}

fn get_chunk_bounding_box(chunk_coord: IVec3) -> BoundingBox {
    let min = (chunk_coord << IVec3::splat(4)).as_float();
    let max = min + Vec3::splat(16.0);

    BoundingBox::new(min, max)
}

fn chunk_coord_to_region_coord(node_position: IVec3) -> IVec3 {
    node_position >> IVec3::new(3, 2, 3)
}

fn position_to_chunk_coord(position: Vec3) -> IVec3 {
    position.floor().as_int().shr(IVec3::splat(4))
}