use crate::api;
use crate::compiler::symtab::ConstantPool;
use crate::config;
use crate::isa;
use crate::types::array;
use crate::types::builtins;
use crate::types::hash;
use crate::types::iter;
use crate::types::object;
use crate::types::th;
use crate::vm::alu;
use crate::vm::errors;
use crate::vm::frames;
use crate::vm::global;
use crate::vm::stack;
use crate::vm::thread;

use std::cell::Ref;
use std::cell::RefCell;
use std::cell::RefMut;
use std::collections::HashMap;
use std::rc::Rc;

use alu::Arithmetic;
use alu::Bitwise;
use alu::Comparision;
use alu::Logical;
use api::Platform;
use array::Array;
use builtins::BuiltinKind;
use config::ENABLE_CONCURRENCY;
use errors::ISAError;
use errors::ISAErrorKind;
use errors::VMError;
use errors::VMErrorKind;
use frames::ExecutionFrame;
use global::GlobalPool;
use hash::HashTable;
use isa::InstructionKind;
use iter::ObjectIterator;
use object::Object;
use stack::DataStack;
use th::ThreadBlock;
use object::AttributeResolver;

pub struct Controls {}

impl Controls {
    pub fn jump(cf: &mut RefMut<ExecutionFrame>, pos: usize) -> Result<usize, VMError> {
        let error = cf.set_ip(pos);
        if error.is_some() {
            return Err(error.unwrap());
        }
        return Ok(pos);
    }

    pub fn jump_not_truthy(
        cf: &mut RefMut<ExecutionFrame>,
        ds: &mut DataStack,
        pos: usize,
    ) -> Result<bool, VMError> {
        let popped_res = ds.pop_object(InstructionKind::INotJump);
        if popped_res.is_err() {
            return Err(popped_res.unwrap_err());
        }

        let popped_obj = popped_res.unwrap();
        if !popped_obj.as_ref().is_true() {
            let jmp_result = Controls::jump(cf, pos);
            if jmp_result.is_err() {
                return Err(jmp_result.unwrap_err());
            }

            return Ok(true);
        }

        return Ok(false);
    }

    pub fn store_global(
        gp: &mut GlobalPool,
        ds: &mut DataStack,
        pos: usize,
    ) -> Result<usize, VMError> {
        let obj_res = ds.pop_object(InstructionKind::IStoreGlobal);
        if obj_res.is_err() {
            return Err(obj_res.unwrap_err());
        }

        let error = gp.set_object(obj_res.unwrap(), pos);
        if error.is_some() {
            return Err(error.unwrap());
        }

        return Ok(pos);
    }

    pub fn load_free(
        ds: &mut DataStack,
        frame: &mut RefMut<ExecutionFrame>,
        idx: usize,
    ) -> Option<VMError> {
        let free_obj_result = frame.get_free(idx, InstructionKind::ILoadFree);
        if free_obj_result.is_err() {
            return Some(free_obj_result.unwrap_err());
        }

        let free_object = free_obj_result.unwrap();
        // push it to the stack:
        let push_result = ds.push_object(free_object, InstructionKind::ILoadFree);

        if push_result.is_err() {
            return Some(push_result.unwrap_err());
        }

        return None;
    }

    pub fn push_objects(objs: Vec<Rc<Object>>, ds: &mut DataStack) -> Option<VMError> {
        let objs_len = objs.len() as i64;
        if ds.stack_pointer + objs_len >= ds.max_size as i64 {
            return Some(VMError::new(
                "Stack Overflow!".to_string(),
                VMErrorKind::DataStackOverflow,
                Some(InstructionKind::ILoadGlobal),
                0,
            ));
        }

        ds.stack.extend(objs);
        ds.stack_pointer += objs_len;
        return None;
    }

    pub fn store_local(
        ds: &mut DataStack,
        pos: usize,
        f: &RefMut<ExecutionFrame>,
    ) -> Result<i64, VMError> {
        let popped_result = ds.pop_object(InstructionKind::IStoreLocal);
        if popped_result.is_err() {
            return Err(popped_result.unwrap_err());
        }

        let bp = f.get_bp();
        ds.stack[bp + pos] = popped_result.unwrap();

        return Ok((bp + pos) as i64);
    }

    pub fn load_local(
        ds: &mut DataStack,
        pos: usize,
        f: &RefMut<ExecutionFrame>,
    ) -> Result<i64, VMError> {
        let bp = f.get_bp();
        let local_object_res = ds.stack.get(bp + pos);
        if local_object_res.is_none() {
            return Err(VMError::new(
                "Stack overflow!".to_string(),
                VMErrorKind::DataStackOverflow,
                Some(InstructionKind::ILoadLocal),
                0,
            ));
        }

        let local_object = local_object_res.unwrap().clone();
        // push the object to stack:
        let push_result = ds.push_object(local_object, InstructionKind::ILoadLocal);
        if push_result.is_err() {
            return Err(push_result.unwrap_err());
        }

        return Ok(push_result.unwrap());
    }

    pub fn load_global(gp: &GlobalPool, ds: &mut DataStack, pos: usize) -> Result<i64, VMError> {
        let object = gp.get(pos);
        if object.is_some() {
            let res = ds.push_object(object.unwrap(), InstructionKind::ILoadGlobal);
            return res;
        }

        return Err(VMError::new(
            format!("Index {} exceeds global pool size {}", pos, gp.max_size),
            VMErrorKind::GlobalPoolSizeExceeded,
            None,
            0,
        ));
    }

    pub fn load_constant(
        cp: &ConstantPool,
        ds: &mut DataStack,
        pos: usize,
    ) -> Result<i64, VMError> {
        let element = cp.get_object(pos).unwrap();
        let result = ds.push_object(element, InstructionKind::IConstant);
        return result;
    }

    pub fn get_binary_operands(
        ds: &mut DataStack,
        inst: &InstructionKind,
    ) -> Result<(Rc<Object>, Rc<Object>), VMError> {
        let right_pop = ds.pop_object(inst.clone());
        if right_pop.is_err() {
            return Err(right_pop.unwrap_err());
        }

        let left_pop = ds.pop_object(inst.clone());
        if right_pop.is_err() {
            return Err(right_pop.unwrap_err());
        }

        return Ok((left_pop.unwrap(), right_pop.unwrap()));
    }

    pub fn execute_binary_op(inst: &InstructionKind, ds: &mut DataStack) -> Option<VMError> {
        let operands_result = Controls::get_binary_operands(ds, inst);
        if operands_result.is_err() {
            return Some(operands_result.unwrap_err());
        }

        let (left, right) = operands_result.unwrap();

        let result = match inst {
            InstructionKind::IAdd => Arithmetic::add(&left, &right),
            InstructionKind::ISub => Arithmetic::sub(&left, &right),
            InstructionKind::IMul => Arithmetic::mul(&left, &right),
            InstructionKind::IDiv => Arithmetic::div(&left, &right),
            InstructionKind::IMod => Arithmetic::modulus(&left, &right),
            InstructionKind::IAnd => Bitwise::and(&left, &right),
            InstructionKind::IOr => Bitwise::or(&left, &right),
            InstructionKind::ILOr => Logical::or(&left, &right),
            InstructionKind::ILAnd => Logical::and(&left, &right),
            InstructionKind::ILGt => Comparision::gt(&left, &right),
            InstructionKind::ILGte => Comparision::gte(&left, &right),
            InstructionKind::ILLt => Comparision::lt(&left, &right),
            InstructionKind::ILLTe => Comparision::lte(&left, &right),
            InstructionKind::ILEq => Comparision::eq(&left, &right),
            InstructionKind::ILNe => Comparision::neq(&left, &right),

            _ => Err(ISAError::new(
                format!("{} is not a binary op", inst.as_string()),
                ISAErrorKind::InvalidOperation,
            )),
        };

        // push result on to stack:
        if result.is_err() {
            return Some(VMError::new_from_isa_error(
                &result.unwrap_err(),
                inst.clone(),
            ));
        }

        let result_obj = result.unwrap();

        // push result to stack:
        let result_push = ds.push_object(result_obj, inst.clone());
        if result_push.is_err() {
            return Some(result_push.unwrap_err());
        }
        return None;
    }

    fn pop_n(
        ds: &mut DataStack,
        n: usize,
        inst: &InstructionKind,
    ) -> Result<Vec<Rc<Object>>, VMError> {
        let mut objs = vec![];

        for _ in 0..n {
            let popped = ds.pop_object(inst.clone());
            if popped.is_err() {
                return Err(popped.unwrap_err());
            }

            let obj = popped.unwrap();
            objs.push(obj);
        }

        return Ok(objs);
    }

    pub fn get_index_value(ds: &mut DataStack) -> Option<VMError> {
        let popped_idx_result = ds.pop_object(InstructionKind::IGetIndex);
        if popped_idx_result.is_err() {
            return Some(popped_idx_result.unwrap_err());
        }

        let popped_left_result = ds.pop_object(InstructionKind::IGetIndex);
        if popped_idx_result.is_err() {
            return Some(popped_left_result.unwrap_err());
        }

        let index_obj = popped_idx_result.unwrap();
        let left_obj = popped_left_result.unwrap();

        // perform indexing:
        let index_result = left_obj.get_indexed(&index_obj);
        if index_result.is_err() {
            return Some(VMError::new(
                index_result.unwrap_err(),
                VMErrorKind::IndexError,
                Some(InstructionKind::IGetIndex),
                0,
            ));
        }

        let push_result = ds.push_object(index_result.unwrap(), InstructionKind::IGetIndex);
        if push_result.is_err() {
            return Some(push_result.unwrap_err());
        }

        return None;
    }

    pub fn load_builtin(ds: &mut DataStack, idx: usize) -> Result<i64, VMError> {
        let builtin_kind = BuiltinKind::get_by_index(idx);
        if builtin_kind.is_none() {
            return Err(VMError::new(
                format!("Unresolved built-in function with index {}", idx),
                VMErrorKind::UnresolvedBuiltinFunction,
                Some(InstructionKind::ILoadBuiltIn),
                0,
            ));
        }

        // push to the stack
        let obj = Rc::new(Object::Builtins(builtin_kind.unwrap()));
        let push_res = ds.push_object(obj, InstructionKind::ILoadBuiltIn);
        if push_res.is_err() {
            return Err(push_res.unwrap_err());
        }

        return Ok(push_res.unwrap());
    }

    pub fn execute_return(
        ds: &mut DataStack,
        frame: &Ref<ExecutionFrame>,
        has_val: bool,
    ) -> Option<VMError> {
        let mut returned_obj: Rc<Object> = Rc::new(Object::Noval);
        if has_val {
            let returned_obj_res = ds.pop_object(InstructionKind::IRetVal);
            if returned_obj_res.is_err() {
                return Some(returned_obj_res.unwrap_err());
            }

            returned_obj = returned_obj_res.unwrap();
        }

        let local_boundary = frame.get_bp();
        // clear off the stack till this point:

        ds.stack.truncate(local_boundary);
        ds.stack_pointer = local_boundary as i64 - 1;

        let push_res = ds.push_object(returned_obj, InstructionKind::IRetVal);
        if push_res.is_err() {
            return Some(push_res.unwrap_err());
        }

        return None;
    }

    pub fn execute_call(
        inst: &InstructionKind,
        ds: &mut DataStack,
        n_args: usize,
        global_pool: &mut GlobalPool,
        constants: &mut ConstantPool,
        platform: &Platform,
        threads: &mut thread::BosonThreads,
    ) -> Result<Option<RefCell<ExecutionFrame>>, VMError> {
        // pop the function:

        let popped = ds.pop_object(inst.clone());
        if popped.is_err() {
            return Err(popped.unwrap_err());
        }

        let popped_obj = popped.unwrap();
        match popped_obj.as_ref() {
            Object::Builtins(func) => {
                // pop the arguments:
                let popped_args = Controls::pop_n(ds, n_args, inst);
                if popped_args.is_err() {
                    return Err(popped_args.unwrap_err());
                }

                let mut args = popped_args.unwrap();
                args.reverse();
                // call the builtin:
                let exec_result = func.exec(args, platform, global_pool, constants, threads);
                if exec_result.is_err() {
                    return Err(VMError::new(
                        exec_result.unwrap_err(),
                        VMErrorKind::BuiltinFunctionError,
                        Some(inst.clone()),
                        0,
                    ));
                }

                let result_obj = exec_result.unwrap();
                let push_res = ds.push_object(result_obj, inst.clone());
                if push_res.is_err() {
                    return Err(push_res.unwrap_err());
                }

                return Ok(None);
            }
            Object::ClosureContext(ctx) => {
                let closure = ctx.as_ref();
                let subroutine = closure.compiled_fn.as_ref();

                if subroutine.num_parameters != n_args {
                    return Err(VMError::new(
                        format!(
                            "Function {} expects {} arguments, given {}",
                            subroutine.name, subroutine.num_parameters, n_args
                        ),
                        VMErrorKind::FunctionArgumentsError,
                        Some(InstructionKind::ICall),
                        0,
                    ));
                }

                let frame_bp = if ds.stack_pointer <= 0 {
                    0
                } else {
                    ds.stack.len() - n_args
                };

                // allocate the stack for local variables and frame:
                let new_frame = ExecutionFrame::new(Rc::new(closure.clone()), frame_bp);

                let n_locals = closure.compiled_fn.num_locals;
                let n_params = closure.compiled_fn.num_parameters;
                let mut local_space = vec![];
                local_space.resize(n_locals - n_params, Rc::new(Object::Noval));

                // push the local space on to the stack
                let push_res = ds.push_objects(InstructionKind::ICall, local_space);
                if push_res.is_err() {
                    return Err(push_res.unwrap_err());
                }

                // set the new stack pointer:
                ds.stack_pointer = (new_frame.base_pointer + n_locals) as i64;
                return Ok(Some(RefCell::new(new_frame)));
            }
            _ => {
                return Err(VMError::new(
                    format!("Cannot call {}", popped_obj.as_ref().describe()),
                    VMErrorKind::StackCorruption,
                    Some(inst.clone()),
                    0,
                ));
            }
        }
    }

    pub fn execute_unary_op(inst: &InstructionKind, ds: &mut DataStack) -> Option<VMError> {
        let pop_result = ds.pop_object(inst.clone());
        if pop_result.is_err() {
            return Some(pop_result.unwrap_err());
        }

        let obj = pop_result.unwrap();

        let result = match inst {
            InstructionKind::INeg => Bitwise::not(&obj),
            InstructionKind::ILNot => Logical::not(&obj),
            _ => Err(ISAError::new(
                format!("{} is not a unary op", inst.as_string()),
                ISAErrorKind::InvalidOperation,
            )),
        };

        if result.is_err() {
            return Some(VMError::new_from_isa_error(
                &result.unwrap_err(),
                inst.clone(),
            ));
        }

        let result_obj = result.unwrap();

        // push result to stack:
        let result_push = ds.push_object(result_obj, inst.clone());
        if result_push.is_err() {
            return Some(result_push.unwrap_err());
        }
        return None;
    }

    pub fn build_array(
        inst: &InstructionKind,
        ds: &mut DataStack,
        length: usize,
    ) -> Result<i64, VMError> {
        let popped_res = Controls::pop_n(ds, length, inst);
        if popped_res.is_err() {
            return Err(popped_res.unwrap_err());
        }

        let mut popped = popped_res.unwrap();
        popped.reverse();

        let array = Array {
            name: "todo".to_string(),
            elements: popped,
        };

        let array_obj = Rc::new(Object::Array(RefCell::new(array)));

        // push the array on to the stack:
        let push_res = ds.push_object(array_obj, inst.clone());
        if push_res.is_err() {
            return Err(push_res.unwrap_err());
        }

        return Ok(push_res.unwrap());
    }

    pub fn build_hash(
        inst: &InstructionKind,
        ds: &mut DataStack,
        length: usize,
    ) -> Result<i64, VMError> {
        let popped_res = Controls::pop_n(ds, length, inst);
        if popped_res.is_err() {
            return Err(popped_res.unwrap_err());
        }

        let mut hash_table = HashMap::new();
        let mut popped = popped_res.unwrap();
        popped.reverse();

        let mut idx = 0;
        while idx < length {
            let key = popped[idx].clone();
            idx += 1;
            let value = popped[idx].clone();
            idx += 1;
            hash_table.insert(key, value);
        }

        let ht = HashTable {
            name: "todo".to_string(),
            entries: hash_table,
        };

        let ht_obj = Rc::new(Object::HashTable(RefCell::new(ht)));
        let push_res = ds.push_object(ht_obj, inst.clone());

        if push_res.is_err() {
            return Err(push_res.unwrap_err());
        }

        return Ok(push_res.unwrap());
    }

    pub fn create_closure(
        ds: &mut DataStack,
        constants: &ConstantPool,
        n_free: usize,
        func_idx: usize,
    ) -> Option<VMError> {
        // pop off the free objects
        let popped_res = Controls::pop_n(ds, n_free, &InstructionKind::IClosure);
        if popped_res.is_err() {
            return Some(popped_res.unwrap_err());
        }
        // get free objects:
        let free_objects = popped_res.unwrap();

        // retrive  the function from constant pool:
        let function_res = constants.get_object(func_idx);
        if function_res.is_none() {
            return Some(VMError::new(
                "Error fetching unknown constant".to_string(),
                VMErrorKind::InvalidGlobalIndex,
                Some(InstructionKind::IClosure),
                0,
            ));
        }

        let function = function_res.unwrap();

        match function.as_ref() {
            Object::Subroutine(sub) => {
                // create a closure:
                let closure_obj = ExecutionFrame::new_closure(sub.clone(), free_objects);
                // load the closure on data-stack:
                let push_res = ds.push_object(closure_obj, InstructionKind::IClosure);
                if push_res.is_err() {
                    return Some(push_res.unwrap_err());
                }
            }
            _ => {
                return Some(VMError::new(
                    format!(
                        "Only functions can be loaded as closure not {}",
                        function.as_ref().get_type()
                    ),
                    VMErrorKind::InvalidGlobalIndex,
                    Some(InstructionKind::IClosure),
                    0,
                ));
            }
        }

        return None;
    }

    pub fn raise_assertion_error(ds: &mut DataStack) -> Option<VMError> {
        let popped_result = ds.pop_object(InstructionKind::IAssertFail);
        if popped_result.is_err() {
            return Some(popped_result.unwrap_err());
        }

        let assert_obj = popped_result.unwrap();

        let assert_fail_str = format!("Assertion Failed: {}", assert_obj.describe());

        return Some(VMError::new(
            assert_fail_str,
            VMErrorKind::AssertionError,
            Some(InstructionKind::IAssertFail),
            0,
        ));
    }

    pub fn create_iter(ds: &mut DataStack) -> Option<VMError> {
        let popped_res = ds.pop_object(InstructionKind::IIter);
        if popped_res.is_err() {
            return Some(popped_res.unwrap_err());
        }

        let popped_object = popped_res.unwrap();

        let iter_res = ObjectIterator::new(popped_object);
        if iter_res.is_err() {
            return Some(VMError::new(
                iter_res.unwrap_err(),
                VMErrorKind::IterationError,
                Some(InstructionKind::IIter),
                0,
            ));
        }

        let iter_object = Rc::new(Object::Iter(RefCell::new(iter_res.unwrap())));
        let push_res = ds.push_object(iter_object, InstructionKind::IIter);
        if push_res.is_err() {
            return Some(push_res.unwrap_err());
        }

        return None;
    }

    pub fn jump_next_iter(
        ds: &mut DataStack,
        jmp_pos: usize,
        frame: &mut RefMut<ExecutionFrame>,
        enumerate: bool,
    ) -> Result<bool, VMError> {
        let top_ref_res = ds.get_top_ref(InstructionKind::IIterNext);
        if top_ref_res.is_err() {
            return Err(top_ref_res.unwrap_err());
        }

        let top_ref = top_ref_res.unwrap();

        match top_ref.as_ref() {
            Object::Iter(iter) => {
                let mut iterator = iter.borrow_mut();

                let mut current_pos = 0;
                if enumerate {
                    current_pos = iterator.get_pos();
                }

                let obj = iterator.next();

                if obj.is_none() {
                    // pop the end
                    drop(iterator);
                    let popped_result = ds.pop_object(InstructionKind::IIterNext);
                    if popped_result.is_err() {
                        return Err(popped_result.unwrap_err());
                    }
                    let result = Controls::jump(frame, jmp_pos);
                    if result.is_err() {
                        return Err(result.unwrap_err());
                    }
                    return Ok(true);
                } else {
                    drop(iterator);
                    let mut push_res = ds.push_object(obj.unwrap(), InstructionKind::IIterNext);
                    if push_res.is_err() {
                        return Err(push_res.unwrap_err());
                    }

                    if enumerate {
                        push_res = ds.push_object(
                            Rc::new(Object::Int(current_pos as i64)),
                            InstructionKind::IIterNext,
                        );
                        if push_res.is_err() {
                            return Err(push_res.unwrap_err());
                        }
                    }

                    return Ok(false);
                }
            }
            _ => {
                return Err(VMError::new(
                    format!("Cannot iterate over {}", top_ref.get_type()),
                    VMErrorKind::IterationError,
                    Some(InstructionKind::IIter),
                    0,
                ));
            }
        }
    }

    pub fn set_indexed(ds: &mut DataStack) -> Option<VMError> {
        let pop_result = Controls::pop_n(ds, 3, &InstructionKind::ISetIndex);

        if pop_result.is_err() {
            return Some(pop_result.unwrap_err());
        }

        // get objects:
        let popped_objects = pop_result.unwrap();
        let popped_right = popped_objects.get(2).unwrap().clone();

        let obj_target = popped_objects.get(1).unwrap();
        let index_target = popped_objects.get(0).unwrap();

        // call set on the object
        let mut new_object = obj_target.as_ref().clone();
        let error = new_object.set_indexed(index_target, popped_right);
        if error.is_some() {
            return Some(VMError::new(
                error.unwrap(),
                VMErrorKind::IndexError,
                Some(InstructionKind::ISetIndex),
                0,
            ));
        }

        // push the object back to stack:
        let push_result = ds.push_object(Rc::new(new_object), InstructionKind::ISetIndex);
        if push_result.is_err() {
            return Some(push_result.unwrap_err());
        }
        return None;
    }

    pub fn execute_thread(
        inst: &InstructionKind,
        ds: &mut DataStack,
        n_args: usize,
        global_pool: &mut GlobalPool,
        constants: &mut ConstantPool,
        platform: &Platform,
        threads: &mut thread::BosonThreads,
        join: bool,
    ) -> Option<VMError> {
        if !ENABLE_CONCURRENCY {
            return Some(VMError::new(
                "BosonVM has concurrency disabled.".to_string(),
                VMErrorKind::IllegalOperation,
                Some(inst.clone()),
                0,
            ));
        }

        // pop the closure:

        let popped_result = ds.pop_object(inst.clone());
        if popped_result.is_err() {
            return Some(popped_result.unwrap_err());
        }

        let popped_obj = popped_result.unwrap();
        match popped_obj.as_ref() {
            Object::ClosureContext(ctx) => {
                let subroutine = ctx.as_ref().compiled_fn.as_ref();
                if subroutine.num_parameters != n_args {
                    return Some(VMError::new(
                        format!(
                            "Function {} expects {} arguments, given {}",
                            subroutine.name, subroutine.num_parameters, n_args
                        ),
                        VMErrorKind::FunctionArgumentsError,
                        Some(inst.clone()),
                        0,
                    ));
                }

                // pop N args from the stack:
                let popped_args = Controls::pop_n(ds, n_args, inst);
                if popped_args.is_err() {
                    return Some(popped_args.unwrap_err());
                }

                let mut args = popped_args.unwrap();
                args.reverse();

                // wrap parameters in a thread-type:
                let thread_params = thread::ThreadParams::new(
                    ctx.clone(),
                    args,
                    global_pool.clone(),
                    constants.clone(),
                );

                let create_result = threads.create_thread_sandbox(thread_params, platform);
                if create_result.is_err() {
                    return Some(VMError::new(
                        create_result.unwrap_err(),
                        VMErrorKind::ThreadCreateError,
                        Some(inst.clone()),
                        0,
                    ));
                }

                let th = create_result.unwrap();

                if !join {
                    // create a thread ID object
                    let thread_obj = ThreadBlock::new(th, subroutine.name.clone());
                    let push_result = ds.push_object(
                        Rc::new(Object::Thread(RefCell::new(thread_obj))),
                        inst.clone(),
                    );
                    if push_result.is_err() {
                        return Some(push_result.unwrap_err());
                    }
                } else {
                    let thread_result = threads.wait_and_return(th);
                    if thread_result.is_err() {
                        return Some(VMError::new(
                            thread_result.unwrap_err(),
                            VMErrorKind::ThreadWaitError,
                            Some(inst.clone()),
                            0,
                        ));
                    }

                    // unwrap the resut object
                    let sandbox_result = thread_result.unwrap().result;
                    if sandbox_result.is_err() {
                        return Some(sandbox_result.unwrap_err());
                    }

                    // push the result to the stack:
                    let push_res = ds.push_object(sandbox_result.unwrap(), inst.clone());
                    if push_res.is_err() {
                        return Some(push_res.unwrap_err());
                    }
                }
            }
            _ => {
                return Some(VMError::new(
                    format!("{} cannot be called as a thread.", popped_obj.get_type()),
                    VMErrorKind::ThreadCreateError,
                    Some(inst.clone()),
                    0,
                ));
            }
        }

        return None;
    }

    pub fn exec_shell(
        inst: &InstructionKind,
        ds: &mut DataStack,
        platform: &Platform,
        gp: &mut GlobalPool,
        c: &mut ConstantPool,
        th: &mut thread::BosonThreads,
        is_raw: bool,
    ) -> Option<VMError> {
        let builtin = if is_raw {
            BuiltinKind::ExecRaw
        } else {
            BuiltinKind::Exec
        };

        let pop_res = ds.pop_object(inst.clone());
        if pop_res.is_err() {
            return Some(pop_res.unwrap_err());
        }

        let popped_obj = pop_res.unwrap();
        match popped_obj.as_ref() {
            Object::Str(_) => {
                // split it to args:
                let shell_fn = platform.sys_shell;
                let mut args: Vec<Rc<Object>> = shell_fn()
                    .split_whitespace()
                    .map(|s| Rc::new(Object::Str(s.to_string())))
                    .collect();

                args.push(popped_obj);
                let exec_result = builtin.exec(args, platform, gp, c, th);
                if exec_result.is_err() {
                    return Some(VMError::new(
                        exec_result.unwrap_err(),
                        VMErrorKind::BuiltinFunctionError,
                        Some(inst.clone()),
                        0,
                    ));
                }

                let push_res = ds.push_object(exec_result.unwrap(), inst.clone());
                if push_res.is_err() {
                    return Some(push_res.unwrap_err());
                }
                return None;
            }

            _ => {
                return Some(VMError::new(
                    format!(
                        "shell requires a string as argument, but got {}",
                        popped_obj.get_type()
                    ),
                    VMErrorKind::TypeError,
                    Some(inst.clone()),
                    0,
                ));
            }
        }
    }

    pub fn get_attr(ds: &mut DataStack, inst: &InstructionKind, n_attrs: usize) -> Option<VMError> {
        let attrs_popped_res = Controls::pop_n(ds, n_attrs, &inst);
        if attrs_popped_res.is_err() {
            return Some(attrs_popped_res.unwrap_err());
        }

        let mut attrs = attrs_popped_res.unwrap();
        attrs.reverse();
        let pop_obj_res = ds.pop_object(inst.clone());

        if pop_obj_res.is_err() {
            return Some(pop_obj_res.unwrap_err());
        }

        let mut obj = pop_obj_res.unwrap();
        // resolve attributes:

        let attr_get_result = obj.resolve_get_attr(&attrs);
        if attr_get_result.is_err() {
            return Some(VMError::new(
                attr_get_result.unwrap_err(),
                VMErrorKind::AttributeError,
                Some(inst.clone()),
                0
            ));
        }

        obj = attr_get_result.unwrap();

        // save the operator to back to the stack
        let push_res = ds.push_object(obj, inst.clone());
        if push_res.is_err() {
            return Some(push_res.unwrap_err());
        }

        return None;
    }

    pub fn call_attr(
        ds: &mut DataStack,
        inst: &InstructionKind,
        n_attrs: usize,
        n_params: usize,
    ) -> Option<VMError> {
        // pop N objects, which act as attributes
        let pop_res = Controls::pop_n(ds, n_attrs, inst);
        if pop_res.is_err() {
            return Some(pop_res.unwrap_err());
        }

        let mut attrs = pop_res.unwrap();
        attrs.reverse();

        // parent assign object:
        let parent_obj_res = ds.pop_object(inst.clone());
        if parent_obj_res.is_err() {
            return Some(parent_obj_res.unwrap_err());
        }

        let parent_obj = parent_obj_res.unwrap();

        // pop all the parameters:
        let param_pop_result = Controls::pop_n(ds, n_params, &inst);
        if param_pop_result.is_err() {
            return Some(param_pop_result.unwrap_err());
        }

        let mut params = param_pop_result.unwrap();
        params.reverse();

        match parent_obj.as_ref() {
           Object::HashTable(ht) => {
               let call_result = ht.borrow_mut().resolve_call_attr(
                   &attrs, &params
               );

               if call_result.is_err() {
                   return Some(VMError::new(
                       call_result.unwrap_err(),
                       VMErrorKind::AttributeError,
                       Some(inst.clone()),
                       0
                   ));
               }

               let object = call_result.unwrap();
               // push the object
               let push_result = ds.push_object(object, inst.clone());
               if push_result.is_err() {
                   return Some(push_result.unwrap_err());
               }
           }
            _ => {
                return Some(VMError::new(
                    format!(
                        "Object of type {} does not support attribute assignment.",
                        parent_obj.get_type()
                    ),
                    VMErrorKind::IllegalOperation,
                    Some(inst.clone()),
                    0,
                ));
            }
        }

        return None;
    }
}
