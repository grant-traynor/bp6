
/// Build prompt using persona plugin system
fn build_prompt_with_persona(
    state: &AgentState,
    persona: &str,
    task: Option<&str>,
    bead_id: Option<&str>,
) -> Result<String, String> {
    use crate::agent::persona::{PersonaContext, PersonaType};

    // Map persona string to PersonaType
    let persona_type = match persona {
        "specialist" => PersonaType::Specialist,
        "product-manager" => PersonaType::ProductManager,
        "qa-engineer" => PersonaType::QaEngineer,
        _ => return Err(format!("Unknown persona: {}", persona)),
    };

    // Get persona plugin from registry
    let persona_plugin = state
        .persona_registry
        .get(persona_type)
        .ok_or_else(|| format!("Persona {:?} not registered", persona_type))?;

    // Get bead and extract information
    let (bead_json, issue_type, role) = if let Some(bid) = bead_id {
        let bead = crate::bd::get_bead_by_id(bid).map_err(|e| e.to_string())?;
        let json = serde_json::to_string_pretty(&bead).ok();
        let issue_type = Some(bead.issue_type.clone());
        let role = get_role_from_bead(&bead);
        (json, issue_type, role)
    } else {
        (None, None, None)
    };

    // Build context for persona plugin
    let context = PersonaContext {
        task: task.map(String::from),
        issue_type,
        bead_id: bead_id.map(String::from),
        role,
    };

    // Get template name from persona plugin
    let template_name = persona_plugin.get_template_name(&context)?;

    // Load template using TemplateLoader
    let template_content = state
        .template_loader
        .load_template(persona_type.as_str(), &template_name)
        .map_err(|e| format!("Failed to load template: {}", e))?;

    // Build final prompt using persona plugin
    let prompt = persona_plugin.build_prompt(template_content, &context, bead_json);

    Ok(prompt)
}
