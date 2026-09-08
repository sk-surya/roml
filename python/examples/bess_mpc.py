"""Rolling-battery MPC example (DESIGN section 8, runnable, self-contained).

Build once, then re-solve over 10 causal forecast gates, applying the
first-interval action each gate. All forecasts are synthetic. Run:
`python python/examples/bess_mpc.py`.
"""

import numpy as np

import roml as rm

N, DT = 24, 0.25
ENERGY_CAP, POWER, EFF, TERMINAL_VALUE = 4.0, 2.0, 0.95, 30.0


def build_model():
    m = rm.Model("rolling-battery")
    price = m.params("price", np.full(N, 50.0))
    initial_energy = m.param("initial_energy", 2.0)
    charge = m.vars("charge", N, ub=POWER)
    discharge = m.vars("discharge", N, ub=POWER)
    energy = m.vars("energy", N + 1, ub=ENERGY_CAP)
    direction = m.vars("direction", N, kind="binary")
    m.add(energy[0] == initial_energy, name="initial_soc")
    m.add(
        energy[1:] == energy[:-1] + DT * (EFF * charge - discharge / EFF),
        name="balance",
    )
    m.add(charge <= POWER * direction, name="charge_mode")
    m.add(discharge <= POWER * (1.0 - direction), name="discharge_mode")
    m.maximize(DT * rm.dot(price, discharge - charge) + TERMINAL_VALUE * energy[-1])
    return m, price, initial_energy, charge, discharge, energy, direction


def main(n_gates=10):
    m, price, initial_energy, charge, discharge, energy, direction = build_model()
    rng = np.random.default_rng(20260907)
    horizon = n_gates + N
    t = np.arange(horizon)
    realized = 50.0 + 60.0 * np.sin(t / 4.0) + rng.normal(0.0, 5.0, size=horizon)
    level = 2.0
    total = 0.0
    with rm.Highs(threads=1, time_limit=2.0) as solver:
        for k in range(n_gates):
            forecasts = realized[k : k + N].copy()
            m.update(price=forecasts, initial_energy=level)
            result = solver.solve(m)
            assert result.has_primal, f"gate {k} has no primal result"
            ch = np.array([result.value(charge[i]) for i in range(N)])
            dh = np.array([result.value(discharge[i]) for i in range(N)])
            en = np.array([result.value(energy[i]) for i in range(N + 1)])
            dr = np.array([result.value(direction[i]) for i in range(N)])
            assert all(min(abs(v), abs(v - 1.0)) <= 1e-6 for v in dr)
            assert abs(en[0] - level) <= 1e-6
            for tt in range(N):
                expect = en[tt] + DT * (EFF * ch[tt] - dh[tt] / EFF)
                assert abs(en[tt + 1] - expect) <= 1e-5
                assert ch[tt] * dh[tt] <= 1e-6
            action = float(dh[0] - ch[0])
            level = float(np.clip(level + DT * (EFF * ch[0] - dh[0] / EFF), 0.0, ENERGY_CAP))
            step_value = float(DT * np.dot(forecasts, dh - ch) + TERMINAL_VALUE * en[-1])
            total += step_value
            print(f"gate {k}: action={action:+.3f} level={level:.3f} value={step_value:.2f}")
    # NOTE: this accumulated total prices forecast energy plus per-gate
    # terminal values; it is a computational smoke signal, not economic
    # evidence of policy quality.
    print(f"total over {n_gates} gates: {total:.2f}")


if __name__ == "__main__":
    main()
