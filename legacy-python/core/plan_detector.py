"""Claude Tank — Detect Claude plan type from organization data."""

from __future__ import annotations

from typing import Any


PLAN_FREE = "Free"
PLAN_PRO = "Pro"
PLAN_MAX_5X = "Max (5x)"
PLAN_MAX_20X = "Max (20x)"
PLAN_TEAM = "Team"
PLAN_ENTERPRISE = "Enterprise"
PLAN_UNKNOWN = "Unknown"


def detect_plan(org_data: dict[str, Any]) -> str:
    """Detect plan type from organization API response.

    The exact field names may vary — this uses heuristics based on
    known response structures from browser extensions.
    """
    # Check for billing/subscription info if available
    billing = org_data.get("billing", {}) or {}
    plan_type = billing.get("plan_type", "").lower()
    if "enterprise" in plan_type:
        return PLAN_ENTERPRISE
    if "team" in plan_type:
        return PLAN_TEAM
    if "max" in plan_type:
        if "20x" in plan_type:
            return PLAN_MAX_20X
        return PLAN_MAX_5X
    if "pro" in plan_type:
        return PLAN_PRO

    # Check capabilities or settings
    capabilities = org_data.get("capabilities", [])
    if isinstance(capabilities, list):
        cap_set = set(capabilities)
        if "enterprise" in cap_set:
            return PLAN_ENTERPRISE
        if "team" in cap_set:
            return PLAN_TEAM

    # Check member count for team detection
    members = org_data.get("members", [])
    if isinstance(members, list) and len(members) > 1:
        return PLAN_TEAM

    # Check active subscription flags
    settings = org_data.get("settings", {}) or {}
    if settings.get("claude_pro_active"):
        return PLAN_PRO

    return PLAN_UNKNOWN


def plan_short_name(plan: str) -> str:
    mapping = {
        PLAN_PRO: "Pro",
        PLAN_MAX_5X: "Max5x",
        PLAN_MAX_20X: "Max20x",
        PLAN_TEAM: "Team",
        PLAN_ENTERPRISE: "Ent",
        PLAN_UNKNOWN: "?",
        PLAN_FREE: "Free",
    }
    return mapping.get(plan, "?")
