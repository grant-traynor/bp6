/// Persona plugin system for AI agent templates
///
/// This module defines the trait-based plugin system for different AI personas
/// (specialist, product-manager, qa-engineer, etc.)
use std::collections::HashMap;
use std::sync::Arc;

/// Quality standards for Product Manager personas
/// These standards are prepended to all PM templates to ensure consistent quality
const QUALITY_STANDARDS: &str = r#"
# MANDATORY Quality Standards for All Beads

**CRITICAL**: These quality standards apply to ALL beads you create, regardless of template or task.

## Creating Features (Epic → Feature decomposition)

EVERY feature MUST have:
- **--acceptance**: Clear success criteria
  - What can users DO when this is done?
  - How do we verify it works? (tests, manual checks)
  - Example: "Users can sign in with Google/GitHub, sessions persist across restarts, auth tests pass with >80% coverage"

- **--design**: Implementation approach
  - Which files/components are involved?
  - What patterns or architecture to follow?
  - Example: "Passport.js for OAuth2, JWT in HTTP-only cookies, UI in src/components/auth/LoginView.tsx"

- **User-value-first description**:
  - Lead with "Users can..." or user benefit
  - Then add technical scope
  - ❌ BAD: "OAuth2 integration with Passport.js"
  - ✅ GOOD: "Users can sign in with Google/GitHub for faster onboarding. Implemented using OAuth2 with Passport.js."

- **Differentiated priority** (0-4):
  - 0 (P0): Critical blocker
  - 1 (P1): High value, foundational
  - 2 (P2): Standard work
  - 3 (P3): Nice-to-have
  - 4 (P4): Backlog
  - **Not all features should be P2** - prioritize based on dependencies and value

## Creating Tasks (Feature → Task decomposition)

EVERY task MUST have:
- **--acceptance**: Specific, testable outcomes
  - What works when done?
  - What tests must pass?
  - Code quality requirements?
  - Example: "User model defined, migrations run, repository pattern in src/data/, unit tests >80% coverage"

- **--design**: Clear implementation plan
  - Specific files to create/modify
  - Patterns to follow from existing code
  - Error handling approach
  - Example: "Create UserRepository.ts with CRUD methods, use Prisma client, follow existing repository pattern"

- **WHAT/WHY/SCOPE description**:
  - WHAT: What are we building?
  - WHY: Why is this needed? (if not obvious)
  - SCOPE: Which files/components?
  - ❌ BAD: "Create database models"
  - ✅ GOOD: "Create User and Profile models with Prisma. Foundation for auth feature to persist user data."

- **Order-based priority**:
  - P1: Foundation (data layer, core services)
  - P2: Standard implementation
  - P3: Polish, optimization
  - Use dependencies (bd dep add) to enforce sequence

## Dependencies

After creating beads:
- **Identify order**: What must complete first?
- **Set explicitly**: Use `bd dep add <dependent> <blocker>`
- **Verify**: Run `bd dep tree` to check flow
- **Foundation → Features → Polish**: Data layer before API before UI

## Quality Checklist

Before running ANY `bd create` command:
- [ ] Description starts with user value (features) or clear WHAT (tasks)
- [ ] --acceptance is specific and testable
- [ ] --design references approach and files
- [ ] Priority reflects actual importance (not default P2)
- [ ] Dependencies will be set after creation

**IF ANY OF THESE ARE MISSING, DO NOT CREATE THE BEAD. FIX IT FIRST.**
"#;

/// Represents different persona types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PersonaType {
    ProductManager,
    QaEngineer,
    Specialist,
    Architect,
}

impl PersonaType {
    pub fn as_str(&self) -> &str {
        match self {
            PersonaType::ProductManager => "product-manager",
            PersonaType::QaEngineer => "qa-engineer",
            PersonaType::Specialist => "specialist",
            PersonaType::Architect => "architect",
        }
    }
}

/// Context information for persona template selection
#[derive(Debug, Clone)]
pub struct PersonaContext {
    pub task: Option<String>,
    pub issue_type: Option<String>,
    pub bead_id: Option<String>,
    pub role: Option<String>,
}

/// Plugin trait for AI personas
///
/// Each persona type implements this trait to provide custom template selection
/// and prompt building logic.
pub trait PersonaPlugin: Send + Sync {
    /// Returns the persona type
    #[allow(dead_code)]
    fn persona_type(&self) -> PersonaType;

    /// Get the template name based on context
    ///
    /// # Arguments
    ///
    /// * `context` - Context information for template selection
    ///
    /// # Returns
    ///
    /// The template file name (without .md extension) to load
    fn get_template_name(&self, context: &PersonaContext) -> Result<String, String>;

    /// Build the complete prompt from template and context
    ///
    /// # Arguments
    ///
    /// * `template_content` - The loaded template content
    /// * `context` - Context information for variable substitution
    /// * `bead_json` - Optional JSON representation of the bead
    ///
    /// # Returns
    ///
    /// The final prompt string ready to send to the agent
    fn build_prompt(
        &self,
        template_content: String,
        context: &PersonaContext,
        bead_json: Option<String>,
    ) -> String {
        let mut prompt = String::new();

        // Inject quality standards for Product Manager personas
        if self.persona_type() == PersonaType::ProductManager {
            prompt.push_str(QUALITY_STANDARDS);
            prompt.push_str("\n\n---\n\n");
        }

        // Add the template content
        prompt.push_str(&template_content);

        // Substitute {{feature_id}} with actual bead_id
        if let Some(bead_id) = &context.bead_id {
            prompt = prompt.replace("{{feature_id}}", bead_id);
        }

        // Append bead JSON context if provided
        if let Some(json) = bead_json {
            prompt.push_str("\nContext JSON:\n```json\n");
            prompt.push_str(&json);
            prompt.push_str("\n```\n");
        }

        prompt
    }

    /// Get variables for template substitution
    #[allow(dead_code)]
    fn get_variables(&self, context: &PersonaContext) -> HashMap<String, String> {
        let mut vars = HashMap::new();
        if let Some(bead_id) = &context.bead_id {
            vars.insert("feature_id".to_string(), bead_id.clone());
        }
        vars
    }
}

/// Registry for persona plugins
pub struct PersonaRegistry {
    personas: HashMap<PersonaType, Arc<dyn PersonaPlugin>>,
}

impl PersonaRegistry {
    /// Create a new empty registry
    pub fn new() -> Self {
        PersonaRegistry {
            personas: HashMap::new(),
        }
    }

    /// Create a registry with all built-in personas
    pub fn with_defaults() -> Self {
        let registry = Self::new();
        registry.register_defaults();
        registry
    }

    /// Register all built-in persona implementations
    pub fn register_defaults(&self) {
        use crate::agent::personas::{ArchitectPersona, ProductManagerPersona, QaEngineerPersona, SpecialistPersona};

        // SAFETY: We're using interior mutability pattern similar to BackendRegistry
        // This is safe because registration only happens during initialization
        unsafe {
            let personas_ptr = &self.personas as *const HashMap<PersonaType, Arc<dyn PersonaPlugin>>
                as *mut HashMap<PersonaType, Arc<dyn PersonaPlugin>>;
            (*personas_ptr).insert(
                PersonaType::ProductManager,
                Arc::new(ProductManagerPersona::new()),
            );
            (*personas_ptr).insert(PersonaType::QaEngineer, Arc::new(QaEngineerPersona::new()));
            (*personas_ptr).insert(PersonaType::Specialist, Arc::new(SpecialistPersona::new()));
            (*personas_ptr).insert(PersonaType::Architect, Arc::new(ArchitectPersona::new()));
        }
    }

    /// Get a persona plugin by type
    pub fn get(&self, persona_type: PersonaType) -> Option<Arc<dyn PersonaPlugin>> {
        self.personas.get(&persona_type).cloned()
    }
}

impl Default for PersonaRegistry {
    fn default() -> Self {
        Self::with_defaults()
    }
}
