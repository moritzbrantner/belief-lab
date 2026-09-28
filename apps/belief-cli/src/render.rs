//! Short deterministic text rendering of an explain [`Report`].

use std::fmt::Write;

use crate::explain::{
    EvidenceUseReport, InferenceReport, InferenceStatus, Outcome, ProducerReport, Report,
    ScoreReport, SourceReport,
};

pub fn render_text(report: &Report) -> String {
    let mut out = String::new();
    // Writing to a String cannot fail.
    let _ = write_report(&mut out, report);
    out
}

fn write_report(out: &mut String, report: &Report) -> std::fmt::Result {
    writeln!(out, "Policy profile: {}", report.policy.profile)?;
    if report.policy.settings.is_empty() {
        writeln!(out, "  settings: none (fail-closed defaults)")?;
    }
    for (key, value) in &report.policy.settings {
        writeln!(out, "  {key}={value}")?;
    }

    if let (Outcome::InputRejected, Some(rejection)) = (report.outcome, &report.rejection) {
        writeln!(out)?;
        writeln!(
            out,
            "Input rejected ({}): {}",
            rejection.stage.as_str(),
            rejection.reason
        )?;
        writeln!(
            out,
            "Failed closed: nothing was imported, inferred, or stored."
        )?;
        return Ok(());
    }

    if let Some(batch) = &report.batch {
        writeln!(out)?;
        writeln!(
            out,
            "Evidence batch {} ({}) exported by {} @ {}",
            batch.revision, batch.schema, batch.exporter.name, batch.exporter.revision
        )?;
        for evidence in &batch.evidence {
            let admission = &evidence.admission;
            if admission.admitted {
                writeln!(
                    out,
                    "  admitted {} [{}] for {}",
                    evidence.id,
                    evidence.class,
                    admission.admitted_purposes.join(", ")
                )?;
            } else {
                writeln!(
                    out,
                    "  rejected {} [{}]: {}",
                    evidence.id,
                    evidence.class,
                    admission.rejection_reasons.join("; ")
                )?;
            }
        }
        if batch.import.imported {
            writeln!(
                out,
                "Import: stored {} evidence record{}.",
                batch.import.stored,
                if batch.import.stored == 1 { "" } else { "s" }
            )?;
        } else {
            writeln!(
                out,
                "Import refused: {}. Nothing was stored.",
                batch.import.reason.as_deref().unwrap_or("not authorized")
            )?;
        }
    }

    if report.inferences.is_empty() {
        writeln!(out)?;
        writeln!(
            out,
            "No judgment script supplied; no inference was requested."
        )?;
    }
    for inference in &report.inferences {
        writeln!(out)?;
        write_inference(out, inference)?;
    }
    Ok(())
}

fn write_inference(out: &mut String, inference: &InferenceReport) -> std::fmt::Result {
    let proposition = &inference.proposition;
    writeln!(
        out,
        "Inference {} [{}]: {} {} {}",
        inference.id,
        inference.class,
        proposition.subject,
        proposition.predicate,
        proposition.object
    )?;
    match (&inference.reason, inference.rule) {
        (Some(reason), _) => writeln!(out, "  {}: {reason}", inference.status.as_str())?,
        (None, Some(rule)) => writeln!(out, "  {} by {rule}", inference.status.as_str())?,
        (None, None) => writeln!(out, "  {}", inference.status.as_str())?,
    }

    if let Some(belief) = &inference.belief {
        writeln!(
            out,
            "  belief {} = {:.3} {}",
            belief.id, belief.value, belief.semantics
        )?;
        writeln!(out, "    meaning: {}", belief.meaning)?;
    }

    let stored = inference.status == InferenceStatus::Derived;
    writeln!(
        out,
        "  judgments{}:",
        if stored { "" } else { " (not stored)" }
    )?;
    for judgment in &inference.judgments {
        write!(
            out,
            "    {} {} {}, group {}",
            judgment.id,
            judgment.outcome,
            score(&judgment.confidence),
            judgment.correlation_group
        )?;
        match judgment.selection {
            Some(selection) => writeln!(out, ", {}", selection.as_str())?,
            None => writeln!(out)?,
        }
        writeln!(out, "      uses {}", uses(&judgment.evidence))?;
    }

    if let Some(authorization) = &inference.authorization {
        writeln!(
            out,
            "  authorized under {} for {}; scopes {}; cross-source join {}",
            authorization.profile,
            authorization.inference_class,
            authorization.source_scopes.join(", "),
            if authorization.cross_source_join {
                "yes"
            } else {
                "no"
            }
        )?;
    }

    if let Some(provenance) = &inference.provenance {
        writeln!(out, "  provenance (read back from the store):")?;
        writeln!(
            out,
            "    belief {} [{}] <- {} run {}",
            provenance.belief,
            provenance.validity,
            provenance.derivation.rule,
            provenance.derivation.inference_run
        )?;
        writeln!(
            out,
            "    claim {} [{}] <- {}",
            provenance.claim.id, provenance.claim.validity, provenance.claim.origin
        )?;
        for judgment in &provenance.judgments {
            writeln!(
                out,
                "    judgment {} [{}] {} {}, spec {}, model {}",
                judgment.id,
                judgment.validity,
                judgment.outcome,
                score(&judgment.confidence),
                judgment.spec,
                judgment.model_revision
            )?;
            for evidence in provenance
                .evidence
                .iter()
                .filter(|evidence| judgment.evidence.contains(&evidence.id))
            {
                writeln!(
                    out,
                    "      evidence {} [{}] {}",
                    evidence.id, evidence.validity, evidence.class
                )?;
                writeln!(out, "        source {}", source(&evidence.source))?;
                writeln!(out, "        producer {}", producer(&evidence.producer))?;
                if !evidence.parents.is_empty() {
                    writeln!(out, "        parents {}", evidence.parents.join(", "))?;
                }
            }
        }
    }
    Ok(())
}

fn score(score: &ScoreReport) -> String {
    format!("{:.3} {}", score.value, score.semantics)
}

fn uses(uses: &[EvidenceUseReport]) -> String {
    uses.iter()
        .map(|item| format!("{} as {}", item.evidence, item.purpose))
        .collect::<Vec<_>>()
        .join(", ")
}

fn source(source: &SourceReport) -> String {
    format!(
        "{} {} {} @ {}",
        source.repository, source.scope_id, source.record_id, source.revision
    )
}

fn producer(producer: &ProducerReport) -> String {
    let mut text = format!("{} @ {}", producer.name, producer.revision);
    if let Some(model) = &producer.model {
        let _ = write!(text, ", model {model}");
    }
    if let Some(config) = &producer.config_hash {
        let _ = write!(text, ", config {config}");
    }
    text
}
