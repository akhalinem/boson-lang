use std::rc::Rc;

use crate::isa;
use crate::types::object;

use isa::errors::ISAError;
use isa::errors::ISAErrorKind;

use object::Object;

pub struct Arithmetic {}
pub struct Bitwise {}

impl Arithmetic {
    pub fn add(left: &Rc<Object>, right: &Rc<Object>) -> Result<Rc<Object>, ISAError> {
        match (left.as_ref(), right.as_ref()) {
            (Object::Int(lval), Object::Int(rval)) => {
                let result = lval + rval;
                return Ok(Rc::new(Object::Int(result)));
            }
            (Object::Int(lval), Object::Float(rval)) => {
                let result = lval.clone() as f64 + rval;
                return Ok(Rc::new(Object::Float(result)));
            }
            (Object::Float(lval), Object::Int(rval)) => {
                let result = lval + rval.clone() as f64;
                return Ok(Rc::new(Object::Float(result)));
            }
            (Object::Float(lval), Object::Float(rval)) => {
                let result = lval + rval;
                return Ok(Rc::new(Object::Float(result)));
            }
            (Object::Str(lval), Object::Str(rval)) => {
                let mut result = lval.clone();
                result.push_str(rval);
                return Ok(Rc::new(Object::Str(result)));
            }
            _ => {
                // throw a panic
                let l_type = left.get_type();
                let r_type = left.get_type();

                return Err(ISAError::new(
                    format!(
                        "Operation Add is not applicable between {} {}",
                        l_type, r_type
                    ),
                    ISAErrorKind::TypeError,
                ));
            }
        }
    }

    pub fn sub(left: &Rc<Object>, right: &Rc<Object>) -> Result<Rc<Object>, ISAError> {
        match (left.as_ref(), right.as_ref()) {
            (Object::Int(lval), Object::Int(rval)) => {
                let result = lval - rval;
                return Ok(Rc::new(Object::Int(result)));
            }
            (Object::Int(lval), Object::Float(rval)) => {
                let result = lval.clone() as f64 - rval;
                return Ok(Rc::new(Object::Float(result)));
            }
            (Object::Float(lval), Object::Int(rval)) => {
                let result = lval - rval.clone() as f64;
                return Ok(Rc::new(Object::Float(result)));
            }
            (Object::Float(lval), Object::Float(rval)) => {
                let result = lval - rval;
                return Ok(Rc::new(Object::Float(result)));
            }
            _ => {
                // throw a panic
                let l_type = left.get_type();
                let r_type = left.get_type();

                return Err(ISAError::new(
                    format!(
                        "Operation Sub is not applicable between {} {}",
                        l_type, r_type
                    ),
                    ISAErrorKind::TypeError,
                ));
            }
        }
    }

    pub fn mul(left: &Rc<Object>, right: &Rc<Object>) -> Result<Rc<Object>, ISAError> {
        match (left.as_ref(), right.as_ref()) {
            (Object::Int(lval), Object::Int(rval)) => {
                let result = lval * rval;
                return Ok(Rc::new(Object::Int(result)));
            }
            (Object::Int(lval), Object::Float(rval)) => {
                let result = lval.clone() as f64 * rval;
                return Ok(Rc::new(Object::Float(result)));
            }
            (Object::Float(lval), Object::Int(rval)) => {
                let result = lval * rval.clone() as f64;
                return Ok(Rc::new(Object::Float(result)));
            }
            (Object::Float(lval), Object::Float(rval)) => {
                let result = lval * rval;
                return Ok(Rc::new(Object::Float(result)));
            }
            _ => {
                // throw a panic
                let l_type = left.get_type();
                let r_type = left.get_type();

                return Err(ISAError::new(
                    format!(
                        "Operation Mul is not applicable between {} {}",
                        l_type, r_type
                    ),
                    ISAErrorKind::TypeError,
                ));
            }
        }
    }

    pub fn div(left: &Rc<Object>, right: &Rc<Object>) -> Result<Rc<Object>, ISAError> {
        match (left.as_ref(), right.as_ref()) {
            (Object::Int(lval), Object::Int(rval)) => {
                if *rval == 0 {
                    return Err(ISAError::new(
                        format!("Divide by zero {}/{}", lval, rval),
                        ISAErrorKind::DivideByZeroError,
                    ));
                }

                let result = lval / rval;
                return Ok(Rc::new(Object::Int(result)));
            }
            (Object::Int(lval), Object::Float(rval)) => {
                if *rval == 0.0 {
                    return Err(ISAError::new(
                        format!("Divide by zero {}/{}", lval, rval),
                        ISAErrorKind::DivideByZeroError,
                    ));
                }

                let result = lval.clone() as f64 / rval;
                return Ok(Rc::new(Object::Float(result)));
            }
            (Object::Float(lval), Object::Int(rval)) => {
                if *rval == 0 {
                    return Err(ISAError::new(
                        format!("Divide by zero {}/{}", lval, rval),
                        ISAErrorKind::DivideByZeroError,
                    ));
                }

                let result = lval * rval.clone() as f64;
                return Ok(Rc::new(Object::Float(result)));
            }
            (Object::Float(lval), Object::Float(rval)) => {
                if *rval == 0.0 {
                    return Err(ISAError::new(
                        format!("Divide by zero {}/{}", lval, rval),
                        ISAErrorKind::DivideByZeroError,
                    ));
                }
                let result = lval / rval;
                return Ok(Rc::new(Object::Float(result)));
            }
            _ => {
                // throw a panic
                let l_type = left.get_type();
                let r_type = left.get_type();

                return Err(ISAError::new(
                    format!(
                        "Operation Div is not applicable between {} {}",
                        l_type, r_type
                    ),
                    ISAErrorKind::TypeError,
                ));
            }
        }
    }

    pub fn modulus(left: &Rc<Object>, right: &Rc<Object>) -> Result<Rc<Object>, ISAError> {
        match (left.as_ref(), right.as_ref()) {
            (Object::Int(lval), Object::Int(rval)) => {
                if *rval == 0 {
                    return Err(ISAError::new(
                        format!("Divide by zero {}/{}", lval, rval),
                        ISAErrorKind::DivideByZeroError,
                    ));
                }

                let result = lval % rval;
                return Ok(Rc::new(Object::Int(result)));
            }
            (Object::Int(lval), Object::Float(rval)) => {
                if *rval == 0.0 {
                    return Err(ISAError::new(
                        format!("Divide by zero {}/{}", lval, rval),
                        ISAErrorKind::DivideByZeroError,
                    ));
                }

                let result = lval.clone() as f64 % rval;
                return Ok(Rc::new(Object::Float(result)));
            }
            (Object::Float(lval), Object::Int(rval)) => {
                if *rval == 0 {
                    return Err(ISAError::new(
                        format!("Divide by zero {}/{}", lval, rval),
                        ISAErrorKind::DivideByZeroError,
                    ));
                }

                let result = lval * rval.clone() as f64;
                return Ok(Rc::new(Object::Float(result)));
            }
            (Object::Float(lval), Object::Float(rval)) => {
                if *rval == 0.0 {
                    return Err(ISAError::new(
                        format!("Divide by zero {}/{}", lval, rval),
                        ISAErrorKind::DivideByZeroError,
                    ));
                }
                let result = lval % rval;
                return Ok(Rc::new(Object::Float(result)));
            }
            _ => {
                // throw a panic
                let l_type = left.get_type();
                let r_type = left.get_type();

                return Err(ISAError::new(
                    format!(
                        "Operation Mod is not applicable between {} {}",
                        l_type, r_type
                    ),
                    ISAErrorKind::TypeError,
                ));
            }
        }
    }
}

impl Bitwise {
    pub fn and(left: &Rc<Object>, right: &Rc<Object>) -> Result<Rc<Object>, ISAError> {
        match (left.as_ref(), right.as_ref()) {
            (Object::Int(lval), Object::Int(rval)) => {
                let result = lval & rval;
                return Ok(Rc::new(Object::Int(result)));
            }
            _ => {
                let l_type = left.get_type();
                let r_type = right.get_type();
                return Err(ISAError::new(
                    format!("Operation And is not applicable between {} and {}", l_type, r_type),
                    ISAErrorKind::TypeError
                ));
            }
        }
    }

    pub fn or(left: &Rc<Object>, right: &Rc<Object>) -> Result<Rc<Object>, ISAError> {
        match (left.as_ref(), right.as_ref()) {
            (Object::Int(lval), Object::Int(rval)) => {
                let result = lval | rval;
                return Ok(Rc::new(Object::Int(result)));
            }
            _ => {
                let l_type = left.get_type();
                let r_type = right.get_type();
                return Err(ISAError::new(
                    format!("Operation Or is not applicable between {} and {}", l_type, r_type),
                    ISAErrorKind::TypeError
                ));
            }
        }
    }

    pub fn not(obj: &Rc<Object>) -> Result<Rc<Object>, ISAError> {
        match obj.as_ref() {
            Object::Int(val) => {
                let result = !val;
                return Ok(Rc::new(Object::Int(result)));
            }
            _ => {
                let val_type = obj.get_type();
                return Err(ISAError::new(
                   format!("Operation Negate is not applicable for {}", val_type) ,
                   ISAErrorKind::TypeError
                ));
            }
        }
    }
}


pub struct Logical {}

impl Logical {

    pub fn or(left: &Rc<Object>, right: &Rc<Object>) -> Result<Rc<Object>, ISAError> {
        let result = left.is_true() || right.is_true();
        return Ok(Rc::new(Object::Bool(result)));
    }

    pub fn and(left: &Rc<Object>, right: &Rc<Object>) -> Result<Rc<Object>, ISAError> {
        let result = left.is_true() && right.is_true();
        return Ok(Rc::new(Object::Bool(result)));
    }

    pub fn not(obj: &Rc<Object>) -> Result<Rc<Object>, ISAError> {
        let result = ! obj.is_true();
        return Ok(Rc::new(Object::Bool(result)));
    }
}
