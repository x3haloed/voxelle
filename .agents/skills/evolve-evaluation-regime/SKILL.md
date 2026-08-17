---
name: evolve-evaluation-regime
description: Use at a designated evaluation checkpoint or when trusted evidence suggests the active evaluation regime is saturated, gameable, miscalibrated, blind to a newly admitted goal invariant, or no longer distinguishes plausible improvements. The regime includes the evaluator, sampling or replay protocol, task weighting, and final scoring rule. Do not use to rewrite the outcome, move goalposts after an unfavorable result, or change evaluation during an unfinished candidate comparison.
---

# Evolve Evaluation Regime

Change how better is judged only at a boundary, using evidence that remains meaningful after the change.

## Separate the objects

Keep these distinct:

- **Outcome:** What must eventually be true for the user or ecosystem. Evaluator evolution must not rewrite it.
- **Goal invariants:** Evidence-backed properties currently believed to be constitutive of that outcome.
- **Evaluation regime `Eₙ`:** The frozen operational procedure currently used to distinguish better work from worse work. It may include objective checks, learned review, human judgment, task selection, replay or challenge sets, weights, thresholds, and aggregation rules.
- **Anchors:** Evidence independent of the active regime that can gate a replacement: fixed contract checks, held-out labeled examples, explicit user decisions, reproducible real-world observations, or other trusted checks.
- **Regime-dependent evidence:** Scores, rankings, pass/fail judgments, or derived conclusions whose meaning depends on `Eₙ`.
- **Raw evidence:** Artifacts, logs, measurements, test output, human observations, and audit records that remain available even if their prior interpretation becomes stale.

A vague virtue such as “quality” is not an evaluation regime. The regime must be explicit enough that an incumbent and challenger can evaluate the same evidence reproducibly.

## Pass the transition gate

Before proposing a replacement:

1. Finish the current candidate observation and pause artifact edits.
2. State the reason for reconsidering `Eₙ`:
   - a designated sparse checkpoint;
   - score saturation or inability to discriminate candidates;
   - a candidate that scores well but fails trusted reality;
   - evidence of reward gaming;
   - disagreement with anchor evidence;
   - a newly admitted goal invariant that `Eₙ` cannot observe;
   - excessive evaluation cost that may justify a cheaper proxy.
3. State `Eₙ` precisely enough to identify what evidence depends on it.
4. Identify an anchor set independent of both `Eₙ` and the proposed challenger.
5. Identify which existing scores, rankings, and conclusions would become stale after replacement.

Do not run this transition on every ordinary iteration. Use natural milestones, explicit evaluator failures, or sparse widening checkpoints.

If no credible anchor exists, retain the incumbent, preserve the uncertainty, and use `$resolve-ambiguity-with-human-evidence` when bounded human judgment can create the needed evidence.

## Construct a challenger

Build `Eₙ₊₁` from decision-relevant evidence such as:

- unresolved evaluator failures;
- newly discovered goal invariants;
- concrete artifacts that passed `Eₙ` but failed trusted reality;
- adversarial or boundary cases;
- repeated evaluator disagreement;
- evaluation-cost measurements.

Separate evidence used to construct or tune `Eₙ₊₁` from evidence used to promote it. Do not tune on the complete anchor set and then treat performance on that same set as independent validation.

Prefer adding a complementary signal over discarding an objective verifier. A learned or subjective evaluator may guide search while executable checks, contracts, or held-out human judgments remain anchors.

Before comparison, predeclare:

- what blind spot or weakness `Eₙ₊₁` is expected to improve;
- which anchors may not regress;
- which trade-offs are acceptable;
- which evidence will decide promotion.

## Compare at the checkpoint

Evaluate the incumbent and challenger against the same held-out anchor bundle.

- Keep hard contract and safety anchors non-negotiable.
- Include evaluator cost, reliability, and reproducibility only when they affect the outcome or the practical ability to continue.
- When results are stochastic, gather enough repeated evidence to avoid promoting a challenger on a noisy point estimate.
- When evidence is tied or materially uncertain, retain the incumbent.
- Never promote a challenger merely because it awards the current artifact a higher score.
- Do not change the anchor rubric during the comparison.

When a fixed anchor is the sole promotion gate, require the challenger to outperform the incumbent on that anchor. When the anchor is only a guardrail and supplemental held-out checks represent the identified blind spot, require no hard-anchor regression and material improvement on the predeclared supplemental checks.

## Perform the transition

On promotion:

1. Freeze `Eₙ₊₁` for the next work epoch.
2. Advance the evaluation-regime epoch identifier.
3. Apply selective erasure:
   - remove or mark stale every score, ranking, leaderboard, or conclusion whose meaning depended on `Eₙ`;
   - retain raw artifacts, logs, measurements, objective checks, and anchor evidence;
   - retain evaluator-independent conclusions.
4. Recompute active summaries only from evidence valid under `Eₙ₊₁`.
5. Re-evaluate prior candidates lazily when they become decision-relevant rather than immediately re-scoring everything.
6. Update the frontier with only the active regime, anchors, and dependency scope.
7. Resume artifact work under the frozen regime.

Do not directly compare scores from different evaluation epochs unless the relevant candidates have been evaluated under the same regime.

## Preserve epistemic limits

A successful evaluator transition supports a better current decision rule; it does not prove global convergence or prove that the anchor fully represents the outcome.

A weak, noisy, or biased anchor can cap evaluator improvement or permit drift. Treat the anchor as a guardrail, not as a complete substitute for the outcome and goal invariants.

Do not evolve the evaluator-replacement rule itself without a separately authorized mechanism and stronger guardrails.