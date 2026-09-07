"""Production planning LP with repricing (MPY-02 golden example).

Build once, solve, update the price parameter in one call, solve again.
Run from an installed wheel: `python python/examples/production.py`.
"""

import roml as rm


def main() -> None:
    m = rm.Model("production")
    x = m.var("x", lb=0.0)
    y = m.var("y", lb=0.0)
    price = m.param("price", 1.0)
    m.add(x + y <= 4.0, name="capacity")
    m.add(x <= 3.0)
    m.maximize(price * x + y)

    with rm.Highs(threads=1, output=False) as solver:
        first = solver.solve(m)
        assert first.is_optimal and first.has_primal
        print(f"price=1: objective={first.objective} x={first.value(x)}")
        assert first.objective == 4.0

        m.update(price=3.0)
        second = solver.solve(m)
        print(f"price=3: objective={second.objective} x={second.value(x)}")
        assert second.value(x) == 3.0
        assert second.objective == 10.0

        # The first result still describes its original revision.
        assert first.objective == 4.0
        assert not first.is_current(m)
        assert second.is_current(m)


if __name__ == "__main__":
    main()
