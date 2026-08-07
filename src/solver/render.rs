//! Deterministic text and Markdown views for infeasibility reports.

use std::fmt;

use crate::solver::infeasibility::{
    InfeasibilityReport, MarkdownInfeasibilityReport, TextInfeasibilityReport,
};

impl fmt::Display for InfeasibilityReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:?}: {} semantic member(s), scope={:?}, guarantee={:?}",
            self.outcome,
            self.members.len(),
            self.scope,
            self.guarantee
        )
    }
}

impl fmt::Display for TextInfeasibilityReport<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let report = self.0;
        writeln!(f, "outcome: {:?}", report.outcome)?;
        writeln!(f, "scope: {:?}", report.scope)?;
        writeln!(f, "completion: {:?}", report.completion)?;
        writeln!(f, "guarantee: {:?}", report.guarantee)?;
        writeln!(f, "oracle_strength: {:?}", report.oracle_strength)?;
        writeln!(f, "model_lineage: {:?}", report.model_lineage)?;
        writeln!(f, "model_instance: {:?}", report.model_instance)?;
        writeln!(f, "model_revision: {:?}", report.model_revision)?;
        writeln!(f, "compilation_id: {:?}", report.compilation_id)?;
        writeln!(
            f,
            "backend: {} {}",
            report.backend.name, report.backend.version
        )?;
        writeln!(f, "provider_chain:")?;
        for provider in &report.provider_chain {
            writeln!(f, "  - {}", provider.name)?;
        }
        writeln!(f, "members:")?;
        for member in &report.members {
            writeln!(
                f,
                "  - atom {:?}: {} = {:?}",
                member.atom_id,
                member.declaration.name.as_deref().unwrap_or("<unnamed>"),
                member.declaration.value
            )?;
        }
        writeln!(f, "technical_evidence:")?;
        writeln!(
            f,
            "  candidate_atoms: {}",
            report.candidate_universe.atom_count
        )?;
        writeln!(f, "  oracle_calls: {}", report.statistics.oracle_calls)?;
        if let Some(native) = &report.native_evidence {
            writeln!(f, "  native_provider: {}", native.provider)?;
        }
        for warning in &report.warnings {
            writeln!(f, "warning: {}", warning.message)?;
        }
        Ok(())
    }
}

impl fmt::Display for MarkdownInfeasibilityReport<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let report = self.0;
        writeln!(f, "# Infeasibility analysis")?;
        writeln!(f, "\n- Outcome: `{:?}`", report.outcome)?;
        writeln!(f, "- Scope: `{:?}`", report.scope)?;
        writeln!(f, "- Completion: `{:?}`", report.completion)?;
        writeln!(f, "- Guarantee: `{:?}`", report.guarantee)?;
        writeln!(
            f,
            "- Backend: `{}` `{}`",
            escape(&report.backend.name),
            escape(&report.backend.version)
        )?;
        writeln!(f, "\n## Semantic members")?;
        if report.members.is_empty() {
            writeln!(f, "\n_None._")?;
        } else {
            for member in &report.members {
                let name = member
                    .declaration
                    .name
                    .as_deref()
                    .map(escape)
                    .unwrap_or_else(|| "<unnamed>".to_string());
                writeln!(f, "\n- Atom `{:?}` — **{}**", member.atom_id, name)?;
            }
        }
        writeln!(f, "\n## Technical evidence")?;
        writeln!(f, "\n- Compilation ID: `{:?}`", report.compilation_id)?;
        writeln!(
            f,
            "- Candidate atoms: `{}`",
            report.candidate_universe.atom_count
        )?;
        writeln!(f, "- Oracle calls: `{}`", report.statistics.oracle_calls)?;
        if let Some(native) = &report.native_evidence {
            writeln!(f, "- Native provider: `{}`", escape(&native.provider))?;
        }
        if !report.warnings.is_empty() {
            writeln!(f, "\n## Warnings")?;
            for warning in &report.warnings {
                writeln!(f, "\n- {}", escape(&warning.message))?;
            }
        }
        Ok(())
    }
}

fn escape(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('`', "\\`")
        .replace('*', "\\*")
        .replace('_', "\\_")
        .replace('[', "\\[")
        .replace(']', "\\]")
}
