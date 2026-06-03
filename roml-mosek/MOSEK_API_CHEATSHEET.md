# MOSEK C API Cheatsheet (for roml developers)

## Callback System

**Register callback:**
```c
MSKrescodee MSK_putcallbackfunc (
    MSKtask_t task,
    MSKcallbackfunc func,
    MSKuserhandle_t handle);
```

**Callback function pointer:**
```c
typedef MSKint32t (MSKAPI * MSKcallbackfunc) (
    MSKtask_t task,             // MOSEK task (call MSK_getxx etc. inside callback)
    MSKuserhandle_t usrptr,     // void* user data
    MSKcallbackcodee caller,    // event code identifying what happened
    const MSKrealt * douinf,   // double info array (indexed by MSK_DINF_*)
    const MSKint32t * intinf,  // int32 info array (indexed by MSK_IINF_*)
    const MSKint64t * lintinf); // int64 info array (indexed by MSK_LIINF_*)
```

**Return value:** `0` = continue optimization, non-zero = terminate with `MSK_RES_TRM_USER_CALLBACK`.

## Key MIO Callback Codes

| Code | Value | Fires On |
|------|-------|----------|
| `MSK_CALLBACK_BEGIN_MIO` | 17 | MIO optimization begins |
| `MSK_CALLBACK_END_MIO` | 54 | MIO optimization ends |
| `MSK_CALLBACK_IM_MIO` | 80 | Intermediate MIO status (during B&B) |
| `MSK_CALLBACK_NEW_INT_MIO` | 91 | **New integer-feasible solution found** |
| `MSK_CALLBACK_RESTART_MIO` | 97 | MIO solver restarts |
| `MSK_CALLBACK_HEARTBEAT` | 75 | Regular interval (polling) |

## Callback Data Access (via douinf / intinf arrays)

### `douinf` — double info (indexed by MSK_DINF_*)

| Constant | Value | Description |
|----------|-------|-------------|
| `MSK_DINF_MIO_OBJ_ABS_GAP` | 35 | Absolute optimality gap |
| `MSK_DINF_MIO_OBJ_BOUND` | 36 | Best dual bound |
| `MSK_DINF_MIO_OBJ_INT` | 37 | Current integer objective (primal bound) |
| `MSK_DINF_MIO_OBJ_REL_GAP` | 38 | Relative optimality gap |

### `intinf` — int32 info (indexed by MSK_IINF_*)

| Constant | Value | Description |
|----------|-------|-------------|
| `MSK_IINF_MIO_NUM_INT_SOLUTIONS` | 47 | Number of integer solutions found |
| `MSK_IINF_MIO_NUM_RELAX` | 48 | Number of relaxations solved |
| `MSK_IINF_MIO_NODE_DEPTH` | 41 | Current node depth |
| `MSK_IINF_MIO_NUM_ACTIVE_NODES` | 42 | Active nodes in branch tree |
| `MSK_IINF_MIO_NUM_BRANCH` | 46 | Branches performed |
| `MSK_IINF_MIO_ABSGAP_SATISFIED` | 20 | Whether abs gap is reached |

## Lazy Constraints / User Cuts

**MOSEK does NOT have native lazy constraint support.** No `CT_LAZY`, no cut callbacks.
The pattern is:

1. Register callback for `MSK_CALLBACK_NEW_INT_MIO`
2. Callback fires when a new integer solution is found
3. Inside callback: call `MSK_getxx` to extract variable values, check for violated constraints
4. If violations: call `MSK_appendcons` + `MSK_putconbound` + `MSK_putaijlist` to add cuts
5. Return **non-zero** from callback → MOSEK terminates with `MSK_RES_TRM_USER_CALLBACK`
6. Call `MSK_optimize` again
7. Repeat until no violations → solution is optimal

MOSEK does **not** auto-continue after cuts added in callback — you must terminate and re-solve.

## Key Functions for Adding Cuts

```c
// Append empty constraints
MSKrescodee MSK_appendcons(MSKtask_t task, MSKint32t num);

// Set constraint bounds
MSKrescodee MSK_putconbound(
    MSKtask_t task, MSKint32t i,
    MSKboundkeye bkc, MSKrealt blc, MSKrealt buc);

// Add coefficients to constraint matrix
MSKrescodee MSK_putaijlist(
    MSKtask_t task, MSKint32t num,
    const MSKint32t * subi,  // row indices
    const MSKint32t * subj,  // column indices
    const MSKrealt * valij); // coefficient values

// Single coefficient (simpler but slower)
MSKrescodee MSK_putaij(
    MSKtask_t task, MSKint32t i, MSKint32t j, MSKrealt valij);
```

## Solution Extraction

```c
// Check if a solution exists
MSKrescodee MSK_solutiondef(
    MSKtask_t task, MSKsoltypee whichsol, MSKint32t * isdef);

// Get primal variable values
MSKrescodee MSK_getxx(
    MSKtask_t task, MSKsoltypee whichsol, MSKrealt * xx);

// Get primal objective value
MSKrescodee MSK_getprimalobj(
    MSKtask_t task, MSKsoltypee whichsol, MSKrealt * obj);

// Get solution status (for SOL_ITG = MIP)
MSKrescodee MSK_getsolsta(
    MSKtask_t task, MSKsoltypee whichsol, MSKsolstae * solsta);

// Get problem status
MSKrescodee MSK_getprosta(
    MSKtask_t task, MSKsoltypee whichsol, MSKprostae * prosta);

// Get all solution data in one call
MSKrescodee MSK_getsolution(
    MSKtask_t task, MSKsoltypee whichsol,
    MSKprostae * prosta, MSKsolstae * solsta,
    MSKstakeye * skc, MSKstakeye * skx, MSKstakeye * skn,
    MSKrealt * xc, MSKrealt * xx, MSKrealt * y,
    MSKrealt * slc, MSKrealt * suc,
    MSKrealt * slx, MSKrealt * sux, MSKrealt * snx);
```

## Solution Types

```c
enum MSKsoltype_enum {
    MSK_SOL_ITR = 0,  // interior point (LP relaxation)
    MSK_SOL_BAS = 1,  // basic solution (simplex)
    MSK_SOL_ITG = 2   // integer (MIP solution)
};
```

## Solution Status

```c
enum MSKsolsta_enum {
    MSK_SOL_STA_UNKNOWN             = 0,
    MSK_SOL_STA_OPTIMAL             = 1,
    MSK_SOL_STA_PRIM_FEAS           = 2,
    MSK_SOL_STA_DUAL_FEAS           = 3,
    MSK_SOL_STA_PRIM_AND_DUAL_FEAS  = 4,
    MSK_SOL_STA_PRIM_INFEAS_CER     = 5,
    MSK_SOL_STA_DUAL_INFEAS_CER     = 6,
    MSK_SOL_STA_PRIM_ILLPOSED_CER   = 7,
    MSK_SOL_STA_DUAL_ILLPOSED_CER   = 8,
    MSK_SOL_STA_INTEGER_OPTIMAL     = 9   // MIP optimal!
};
```

## Variable / Constraint Management

```c
// Variable bounds
MSKrescodee MSK_putvarbound(
    MSKtask_t task, MSKint32t i,
    MSKboundkeye bk, MSKrealt bl, MSKrealt bu);

// Variable type (continuous/integer)
MSKrescodee MSK_putvartype(
    MSKtask_t task, MSKint32t j, MSKvariabletypee vt);

// Add variables
MSKrescodee MSK_appendvars(MSKtask_t task, MSKint32t num);

// Delete variables/constraints (removes by index, shifts remaining)
MSKrescodee MSK_removevars(MSKtask_t task, MSKint32t num, const MSKint32t * subset);
MSKrescodee MSK_removecons(MSKtask_t task, MSKint32t num, const MSKint32t * subset);

// Objective
MSKrescodee MSK_putcj(MSKtask_t task, MSKint32t j, MSKrealt cj);
MSKrescodee MSK_putobjsense(MSKtask_t task, MSKobjsensee sense);
```

## Bound Keys

```c
enum MSKboundkey_enum {
    MSK_BK_LO = 0,  // lower:   bl <= ax
    MSK_BK_UP = 1,  // upper:   ax <= bu
    MSK_BK_FX = 2,  // fixed:   bl == ax == bu
    MSK_BK_FR = 3,  // free:    -inf < ax < +inf
    MSK_BK_RA = 4   // ranged:  bl <= ax <= bu
};
```

## Optimize Functions

```c
// Plain optimize
MSKrescodee MSK_optimize(MSKtask_t task);

// Optimize with termination code
MSKrescodee MSK_optimizetrm(MSKtask_t task, MSKrescodee * trmcode);
```

## roml Integration Notes

### What's already bound in `roml-mosek/src/ffi.rs`

Lifecycle: `MSK_makeenv`, `MSK_deleteenv`, `MSK_maketask`, `MSK_deletetask`
Parameters: `MSK_putintparam`
Variables: `MSK_appendvars`, `MSK_putvarbound`, `MSK_putvartype`, `MSK_removevars`
Constraints: `MSK_appendcons`, `MSK_putconbound`, `MSK_putaij`, `MSK_removecons`
Objective: `MSK_putcj`, `MSK_putobjsense`
Solve: `MSK_optimize`
Solution: `MSK_getsolsta`, `MSK_getprosta`, `MSK_getxx`, `MSK_gety`, `MSK_getslx`, `MSK_getsux`, `MSK_getprimalobj`
Logging: `MSK_linkfunctotaskstream`

### What needs to be added for callback support

1. `MSK_putcallbackfunc` — register the callback
2. `MSKcallbackfunc` — the C function pointer type
3. Callback code constants: `CALLBACK_NEW_INT_MIO=91`, `CALLBACK_IM_MIO=80`, `CALLBACK_BEGIN_MIO=17`, `CALLBACK_END_MIO=54`
4. Info index constants: `DINF_MIO_OBJ_INT=37`, `DINF_MIO_OBJ_BOUND=36`, `DINF_MIO_OBJ_ABS_GAP=35`, `DINF_MIO_OBJ_REL_GAP=38`, `IINF_MIO_NUM_INT_SOLUTIONS=47`
5. `MSK_putaijlist` — for efficient coefficient addition in callbacks
6. `MSK_solutiondef` — check if solution exists
7. Termination code: `RES_TRM_USER_CALLBACK=100007`

### Adapter pattern for solve-with-callbacks

```rust
fn solve(&mut self) -> Result<SolverStatus, SolverError> {
    if let Some(handler) = self.callback_handler.take() {
        self.solve_with_callbacks(handler)
    } else {
        self.solve_plain()
    }
}

fn solve_with_callbacks(&mut self, handler: Box<dyn CallbackHandler>) -> ... {
    // Build CallbackState with maps
    // Register MSK_putcallbackfunc with trampoline
    loop {
        let ret = unsafe { MSK_optimize(self.task) };
        if ret != RES_TRM_USER_CALLBACK { break; }
        // Cuts were added, callback triggered re-solve
        // Continue loop (cuts were applied inside callback)
    }
    // Extract solution
    // Restore handler
}
```

## See Also

- Full MOSEK API: `mosek.h` in the MOSEK installation
- Existing roml-mosek FFI: `roml-mosek/src/ffi.rs`
- Existing adapter: `roml-mosek/src/adapter.rs`
- roml callback types: `roml/src/solver/callback.rs`
- HiGHS callback implementation (reference): `roml-highs/src/adapter.rs`
