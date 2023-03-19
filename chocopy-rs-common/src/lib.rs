use std::ptr::*;

pub const POINTER_SIZE: u32 = 8;
pub const FUNCTION_POINTER_SIZE: u32 = 8;

#[repr(i32)]
pub enum TypeTag {
    // Normal objects
    Other = 0,
    Int = 1,
    Bool = 2,

    // Array objects
    Str = 3,
    PlainList = -1,
    RefList = -2,
}

#[repr(C)]
pub struct Prototype {
    pub size: i32, // >= 0 for normal object. < 0 for array object
    pub tag: TypeTag,
    pub map: *const u8,
    // followed by other method pointers
}
pub const PROTOTYPE_SIZE_OFFSET: u32 = 0;
pub const PROTOTYPE_TAG_OFFSET: u32 = PROTOTYPE_SIZE_OFFSET + 4;
pub const PROTOTYPE_MAP_OFFSET: u32 = PROTOTYPE_TAG_OFFSET + 4;
pub const PROTOTYPE_INIT_OFFSET: u32 = PROTOTYPE_MAP_OFFSET + FUNCTION_POINTER_SIZE;
pub const OBJECT_PROTOTYPE_SIZE: u32 = PROTOTYPE_INIT_OFFSET + FUNCTION_POINTER_SIZE;
pub const PROTOTYPE_HEADER_MEMBER_COUNT: u32 = 3;

#[repr(C)]
pub struct Object {
    pub prototype: *const Prototype,
    pub gc_count: u64,
    pub gc_next: Option<NonNull<Object>>,
    // followed by attributes
}

pub const OBJECT_PROTOTYPE_OFFSET: u32 = 0;
pub const OBJECT_GC_COUNT_OFFSET: u32 = OBJECT_PROTOTYPE_OFFSET + 8;
pub const OBJECT_GC_NEXT_OFFSET: u32 = OBJECT_GC_COUNT_OFFSET + 8;
pub const OBJECT_ATTRIBUTE_OFFSET: u32 = OBJECT_GC_NEXT_OFFSET + 8;
pub const OBJECT_HEADER_MEMBER_COUNT: u32 = 3;

#[repr(C)]
pub struct ArrayObject {
    pub object: Object,
    pub len: u64,
}

pub const ARRAY_LEN_OFFSET: u32 = OBJECT_ATTRIBUTE_OFFSET;
pub const ARRAY_ELEMENT_OFFSET: u32 = ARRAY_LEN_OFFSET + 8;

#[repr(C)]
pub struct InitParam {
    pub bottom_frame: *const u64,
    pub global_section: *const u64,
    pub global_size: u64,
    pub global_map: *const u8,
    pub str_prototype: *const Prototype,
}

pub const BOTTOM_FRAME_OFFSET: u32 = 0;
pub const GLOBAL_SECTION_OFFSET: u32 = BOTTOM_FRAME_OFFSET + POINTER_SIZE;
pub const GLOBAL_SIZE_OFFSET: u32 = GLOBAL_SECTION_OFFSET + POINTER_SIZE;
pub const GLOBAL_MAP_OFFSET: u32 = GLOBAL_SIZE_OFFSET + 8;
pub const STR_PROTOTYPE_OFFSET: u32 = GLOBAL_MAP_OFFSET + POINTER_SIZE;
pub const INIT_PARAM_SIZE: u32 = std::mem::size_of::<InitParam>() as u32;
