//! Ergonomic operator algebra over the L1 arrays (MIR-04, IR-24).
//!
//! These operators are the ergonomic surface; the fallible `try_*` methods
//! remain the typed-error surface. An operator on mismatched ownership or shape
//! is a programming error and panics with a clear message (use
//! [`LinArray::try_add`]/[`LinArray::try_sub`] for fallible composition); a
//! scalar shift of a parameterized constant is likewise only available through
//! [`LinArray::try_shift`].
//!
//! Variable and linear arrays mix freely: `η * charge - discharge / η`,
//! `energy_prev + dt * (...)`, and `-x` all yield a [`LinArray`].

use std::ops::{Add, Div, Mul, Neg, Sub};

use crate::modeling::{LinArray, VarArray};

fn add_lin(left: LinArray, right: LinArray) -> LinArray {
    left.try_add(right)
        .expect("LinArray + LinArray: owner/shape mismatch (use try_add)")
}

fn sub_lin(left: LinArray, right: LinArray) -> LinArray {
    left.try_sub(right)
        .expect("LinArray - LinArray: owner/shape mismatch (use try_sub)")
}

fn shift_lin(array: LinArray, delta: f64) -> LinArray {
    array
        .try_shift(delta)
        .expect("LinArray scalar shift: unsupported constant (use try_shift)")
}

macro_rules! impl_lin_binop {
    ($trait:ident, $method:ident, $combine:ident) => {
        impl $trait for LinArray {
            type Output = LinArray;
            fn $method(self, rhs: LinArray) -> LinArray {
                $combine(self, rhs)
            }
        }
        impl $trait<&LinArray> for LinArray {
            type Output = LinArray;
            fn $method(self, rhs: &LinArray) -> LinArray {
                $combine(self, rhs.clone())
            }
        }
        impl $trait<LinArray> for &LinArray {
            type Output = LinArray;
            fn $method(self, rhs: LinArray) -> LinArray {
                $combine(self.clone(), rhs)
            }
        }
        impl $trait<&LinArray> for &LinArray {
            type Output = LinArray;
            fn $method(self, rhs: &LinArray) -> LinArray {
                $combine(self.clone(), rhs.clone())
            }
        }
    };
}

impl_lin_binop!(Add, add, add_lin);
impl_lin_binop!(Sub, sub, sub_lin);

impl Mul<f64> for LinArray {
    type Output = LinArray;
    fn mul(self, rhs: f64) -> LinArray {
        self.scaled(rhs)
    }
}

impl Mul<f64> for &LinArray {
    type Output = LinArray;
    fn mul(self, rhs: f64) -> LinArray {
        self.clone().scaled(rhs)
    }
}

impl Mul<LinArray> for f64 {
    type Output = LinArray;
    fn mul(self, rhs: LinArray) -> LinArray {
        rhs.scaled(self)
    }
}

impl Mul<&LinArray> for f64 {
    type Output = LinArray;
    fn mul(self, rhs: &LinArray) -> LinArray {
        rhs.clone().scaled(self)
    }
}

impl Div<f64> for LinArray {
    type Output = LinArray;
    fn div(self, rhs: f64) -> LinArray {
        self.scaled(1.0 / rhs)
    }
}

impl Div<f64> for &LinArray {
    type Output = LinArray;
    fn div(self, rhs: f64) -> LinArray {
        self.clone().scaled(1.0 / rhs)
    }
}

impl Neg for LinArray {
    type Output = LinArray;
    fn neg(self) -> LinArray {
        self.scaled(-1.0)
    }
}

impl Neg for &LinArray {
    type Output = LinArray;
    fn neg(self) -> LinArray {
        self.clone().scaled(-1.0)
    }
}

impl Add<f64> for LinArray {
    type Output = LinArray;
    fn add(self, rhs: f64) -> LinArray {
        shift_lin(self, rhs)
    }
}

impl Add<f64> for &LinArray {
    type Output = LinArray;
    fn add(self, rhs: f64) -> LinArray {
        shift_lin(self.clone(), rhs)
    }
}

impl Sub<f64> for LinArray {
    type Output = LinArray;
    fn sub(self, rhs: f64) -> LinArray {
        shift_lin(self, -rhs)
    }
}

impl Sub<f64> for &LinArray {
    type Output = LinArray;
    fn sub(self, rhs: f64) -> LinArray {
        shift_lin(self.clone(), -rhs)
    }
}

macro_rules! impl_var_lin_binop {
    ($trait:ident, $method:ident, $combine:ident) => {
        impl $trait for VarArray {
            type Output = LinArray;
            fn $method(self, rhs: VarArray) -> LinArray {
                $combine(self.lin(), rhs.lin())
            }
        }
        impl $trait<&VarArray> for VarArray {
            type Output = LinArray;
            fn $method(self, rhs: &VarArray) -> LinArray {
                $combine(self.lin(), rhs.lin())
            }
        }
        impl $trait<VarArray> for &VarArray {
            type Output = LinArray;
            fn $method(self, rhs: VarArray) -> LinArray {
                $combine(self.lin(), rhs.lin())
            }
        }
        impl $trait<&VarArray> for &VarArray {
            type Output = LinArray;
            fn $method(self, rhs: &VarArray) -> LinArray {
                $combine(self.lin(), rhs.lin())
            }
        }
        impl $trait<LinArray> for VarArray {
            type Output = LinArray;
            fn $method(self, rhs: LinArray) -> LinArray {
                $combine(self.lin(), rhs)
            }
        }
        impl $trait<&LinArray> for VarArray {
            type Output = LinArray;
            fn $method(self, rhs: &LinArray) -> LinArray {
                $combine(self.lin(), rhs.clone())
            }
        }
        impl $trait<LinArray> for &VarArray {
            type Output = LinArray;
            fn $method(self, rhs: LinArray) -> LinArray {
                $combine(self.lin(), rhs)
            }
        }
        impl $trait<&LinArray> for &VarArray {
            type Output = LinArray;
            fn $method(self, rhs: &LinArray) -> LinArray {
                $combine(self.lin(), rhs.clone())
            }
        }
        impl $trait<VarArray> for LinArray {
            type Output = LinArray;
            fn $method(self, rhs: VarArray) -> LinArray {
                $combine(self, rhs.lin())
            }
        }
        impl $trait<&VarArray> for LinArray {
            type Output = LinArray;
            fn $method(self, rhs: &VarArray) -> LinArray {
                $combine(self, rhs.lin())
            }
        }
        impl $trait<VarArray> for &LinArray {
            type Output = LinArray;
            fn $method(self, rhs: VarArray) -> LinArray {
                $combine(self.clone(), rhs.lin())
            }
        }
        impl $trait<&VarArray> for &LinArray {
            type Output = LinArray;
            fn $method(self, rhs: &VarArray) -> LinArray {
                $combine(self.clone(), rhs.lin())
            }
        }
    };
}

impl_var_lin_binop!(Add, add, add_lin);
impl_var_lin_binop!(Sub, sub, sub_lin);

impl Mul<f64> for VarArray {
    type Output = LinArray;
    fn mul(self, rhs: f64) -> LinArray {
        self.lin().scaled(rhs)
    }
}

impl Mul<f64> for &VarArray {
    type Output = LinArray;
    fn mul(self, rhs: f64) -> LinArray {
        self.lin().scaled(rhs)
    }
}

impl Mul<VarArray> for f64 {
    type Output = LinArray;
    fn mul(self, rhs: VarArray) -> LinArray {
        rhs.lin().scaled(self)
    }
}

impl Mul<&VarArray> for f64 {
    type Output = LinArray;
    fn mul(self, rhs: &VarArray) -> LinArray {
        rhs.lin().scaled(self)
    }
}

impl Div<f64> for VarArray {
    type Output = LinArray;
    fn div(self, rhs: f64) -> LinArray {
        self.lin().scaled(1.0 / rhs)
    }
}

impl Div<f64> for &VarArray {
    type Output = LinArray;
    fn div(self, rhs: f64) -> LinArray {
        self.lin().scaled(1.0 / rhs)
    }
}

impl Neg for VarArray {
    type Output = LinArray;
    fn neg(self) -> LinArray {
        self.lin().scaled(-1.0)
    }
}

impl Neg for &VarArray {
    type Output = LinArray;
    fn neg(self) -> LinArray {
        self.lin().scaled(-1.0)
    }
}

impl Add<f64> for VarArray {
    type Output = LinArray;
    fn add(self, rhs: f64) -> LinArray {
        shift_lin(self.lin(), rhs)
    }
}

impl Add<f64> for &VarArray {
    type Output = LinArray;
    fn add(self, rhs: f64) -> LinArray {
        shift_lin(self.lin(), rhs)
    }
}

impl Sub<f64> for VarArray {
    type Output = LinArray;
    fn sub(self, rhs: f64) -> LinArray {
        shift_lin(self.lin(), -rhs)
    }
}

impl Sub<f64> for &VarArray {
    type Output = LinArray;
    fn sub(self, rhs: f64) -> LinArray {
        shift_lin(self.lin(), -rhs)
    }
}
