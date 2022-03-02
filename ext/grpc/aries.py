from typing import Optional
import unified_planning
import unified_planning.interop
from unified_planning.solvers.solver import Solver


from libaries import Aries


class Aries(Solver):
    """Implements the plugin that uses Aries solver"""

    def __init__(self, **kwargs):
        if len(kwargs) > 0:
            raise

    @staticmethod
    def name() -> str:
        return "aries"

    @staticmethod
    def is_oneshot_planner() -> bool:
        return True

    @staticmethod
    def is_plan_validator() -> bool:
        return False

    @staticmethod
    def is_grounder() -> bool:
        return False

    @staticmethod
    def supports(problem_kind: "unified_planning.model.ProblemKind") -> bool:
        supported_kind = unified_planning.model.ProblemKind()
        supported_kind.set_typing("FLAT_TYPING")  # type: ignore
        supported_kind.set_typing("HIERARCHICAL_TYPING")  # type: ignore
        supported_kind.set_conditions_kind("EQUALITY")  # type: ignore
        supported_kind.set_conditions_kind("UNIVERSAL_CONDITIONS")  # type: ignore
        return problem_kind <= supported_kind

    def solve(
        self, problem: "unified_planning.model.Problem"
    ) -> Optional["unified_planning.plan.Plan"]:
        # TODO: Implement PyInit Libraries for UP.Problem and UP.Answer
        answer = Aries.solve(problem)
        if answer.answer.status is not None:
            raise Exception("Aries failed to solve the problem")
        else:
            return answer.answer.plan

    def destroy(self):
        pass
