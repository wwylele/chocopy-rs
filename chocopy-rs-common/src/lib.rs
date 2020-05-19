pub const POINTER_SIZE: u32 = 8;
pub const FUNCTION_POINTER_SIZE: u32 = 8;

#[repr(i32)]
pub enum TypeTag {
    Other = 0,
    Int = 1,
    Bool = 2,
    Str = 3,
    List = -1,
}

#[repr(C)]
pub struct Prototype {
    pub size: i32,
    pub tag: TypeTag,
    pub map: *const u8,
    pub dtor: unsafe extern "C" fn(*mut Object),
    // followed by other method pointers
}
pub const PROTOTYPE_SIZE_OFFSET: u32 = 0;
pub const PROTOTYPE_TAG_OFFSET: u32 = PROTOTYPE_SIZE_OFFSET + 4;
pub const PROTOTYPE_MAP_OFFSET: u32 = PROTOTYPE_TAG_OFFSET + 4;
pub const PROTOTYPE_DTOR_OFFSET: u32 = PROTOTYPE_MAP_OFFSET + FUNCTION_POINTER_SIZE;
pub const PROTOTYPE_INIT_OFFSET: u32 = PROTOTYPE_DTOR_OFFSET + FUNCTION_POINTER_SIZE;
pub const OBJECT_PROTOTYPE_SIZE: u32 = PROTOTYPE_INIT_OFFSET + FUNCTION_POINTER_SIZE;
pub const PROTOTYPE_HEADER_MEMBER_COUNT: u32 = 3;

#[repr(C)]
pub struct Object {
    pub prototype: *const Prototype,
    pub ref_count: u64,
    // followed by attributes
}

pub const OBJECT_PROTOTYPE_OFFSET: u32 = 0;
pub const OBJECT_REF_COUNT_OFFSET: u32 = OBJECT_PROTOTYPE_OFFSET + POINTER_SIZE;
pub const OBJECT_ATTRIBUTE_OFFSET: u32 = OBJECT_REF_COUNT_OFFSET + 8;
pub const OBJECT_HEADER_MEMBER_COUNT: u32 = 2;

#[repr(C)]
pub struct ArrayObject {
    pub object: Object,
    pub len: u64,
}

pub const ARRAY_LEN_OFFSET: u32 = OBJECT_ATTRIBUTE_OFFSET;
pub const ARRAY_ELEMENT_OFFSET: u32 = ARRAY_LEN_OFFSET + 8;
