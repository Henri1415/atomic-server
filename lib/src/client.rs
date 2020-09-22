//! Functions for interacting with an Atomic Server
use crate::{delta::Delta, errors::AtomicResult, parse::parse_ad3, ResourceString};

/// Fetches a resource, makes sure its subject matches.
/// Only adds atoms with matching subjects match.
pub fn fetch_resource(subject: &str) -> AtomicResult<ResourceString> {
    let resp = ureq::get(&subject)
        .set("Accept", crate::parse::AD3_MIME)
        .timeout_read(500)
        .call();
    if resp.status() != 200 {
        return Err(format!("Could not fetch {}. Status: {}", subject, resp.status()).into());
    }
    let body = &resp
        .into_string()
        .map_err(|e| format!("Could not parse response {}: {}", subject, e))?;
    let atoms = parse_ad3(body).map_err(|e| format!("Error parsing body of {}: {}", subject, e))?;
    let mut resource = ResourceString::new();
    for atom in atoms {
        if atom.subject == subject {
            resource.insert(atom.property, atom.value);
        }
    }
    if resource.is_empty() {
        return Err("No valid atoms in resource".into());
    }
    Ok(resource)
}

/// Posts a delta to an endpoint
pub fn post_delta(endpoint: &str, delta: Delta) -> AtomicResult<()> {
    let resp = ureq::post(&endpoint)
        .set("Accept", crate::parse::AD3_MIME)
        .timeout_read(500)
        .call();
    Ok(())
}