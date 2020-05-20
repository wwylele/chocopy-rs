use super::*;

unsafe fn read_i32_le(p: *const u8) -> i32 {
    let mut buf = [0; 4];
    std::ptr::copy_nonoverlapping(p, buf.as_mut_ptr(), 4);
    i32::from_le_bytes(buf)
}

unsafe fn get_ref_map(rip: *const u8) -> *const u8 {
    let offset = read_i32_le(rip.offset(3));
    rip.offset((offset + 7) as isize)
}

unsafe fn walk(var: *const u64) {
    if *var == 0 {
        return;
    }

    let object = *var as *mut Object;
    if (*object).gc_count == GC_COUNTER.load(Ordering::SeqCst) {
        return;
    }
    (*object).gc_count = GC_COUNTER.load(Ordering::SeqCst);

    match (*(*object).prototype).tag {
        TypeTag::Other => {
            let len = ((*(*object).prototype).size / 8) as usize;
            let ref_map = (*(*object).prototype).map;
            for i in 0..len {
                let flag = *ref_map.add(i / 8) & (1 << (i % 8));
                if flag != 0 {
                    walk((object.add(1) as *const u64).add(i));
                }
            }
        }
        TypeTag::List if (*(*object).prototype).size == -8 => {
            let list = object as *mut ArrayObject;
            for i in 0..(*list).len {
                walk((list.add(1) as *const u64).add(i as usize));
            }
        }
        _ => (),
    }
}

pub unsafe fn collect(rbp: *const u64, rsp: *const u64) {
    GC_COUNTER.fetch_add(1, Ordering::SeqCst);
    let init_param = INIT_PARAM.load(Ordering::SeqCst).as_ref().unwrap();
    let mut rip = *rsp.offset(-1) as *const u8;
    let mut current_frame = rbp;
    loop {
        let ref_map = get_ref_map(rip);
        let min_index = read_i32_le(ref_map);
        let max_index = read_i32_le(ref_map.offset(4));
        for index in min_index..=max_index {
            let map_index = (index - min_index) as usize;
            let flag = *ref_map.add(8 + map_index / 8) & (1 << (map_index % 8));
            if flag != 0 {
                walk(current_frame.offset(index as isize));
            }
        }

        if current_frame == init_param.bottom_frame {
            break;
        }
        rip = *current_frame.offset(1) as *const u8;
        current_frame = *current_frame as *const u64;
    }

    for index in 0..init_param.global_size / 8 {
        let index = index as usize;
        let flag = *init_param.global_map.add(index / 8) & (1 << (index % 8));
        if flag != 0 {
            walk(init_param.global_section.add(index));
        }
    }

    OBJECT_STORE.with(|object_store| {
        object_store.borrow_mut().retain(|boxed| {
            let object = boxed.as_ptr() as *const Object;
            (*object).gc_count == GC_COUNTER.load(Ordering::SeqCst)
        })
    });
}
