//! Mockspace lint pack for the hilavitkutin / arvo / notko / clause stack.
//!
//! Consumed by each repo's `mockspace.toml` via:
//!
//! ```toml
//! [lint-crates]
//! mockspace-hilavitkutin-stack-lints = { path = "../mockspace-hilavitkutin-stack-lints" }
//! ```
//!
//! Every lint is emitted from one place so severity policy stays in sync
//! across arvo, hilavitkutin, clause, and notko.

mod util;

pub mod lints {
    //! Individual lint rules. Each is a unit struct that implements
    //! `mockspace_lint_rules::Lint` or `CrossCrateLint`.

    pub mod no_alloc;
    pub mod no_std;
    pub mod no_bare_option;
    pub mod no_bare_result;
    pub mod no_bare_numeric;
    pub mod no_bare_string;
    pub mod no_bare_static_str;
    pub mod no_dyn_dispatch;
    pub mod no_runtime_spawn;
    pub mod no_runtime_registration;
    pub mod no_public_raw_field;
    pub mod no_vec_in_trait_sig;
    pub mod strategy_marker_required;
    pub mod semantic_alias_nudge;
    pub mod trait_first_signatures;
    pub mod arvo_types_only;
    pub mod lint_allow_requires_task_id;
    pub mod writing_style;
}

use lints::{
    arvo_types_only::ArvoTypesOnly, lint_allow_requires_task_id::LintAllowRequiresTaskId,
    no_alloc::NoAlloc, no_bare_numeric::NoBareNumeric, no_bare_option::NoBareOption,
    no_bare_result::NoBareResult, no_bare_static_str::NoBareStaticStr,
    no_bare_string::NoBareString, no_dyn_dispatch::NoDynDispatch,
    no_public_raw_field::NoPublicRawField, no_runtime_registration::NoRuntimeRegistration,
    no_runtime_spawn::NoRuntimeSpawn, no_std::NoStd, no_vec_in_trait_sig::NoVecInTraitSig,
    semantic_alias_nudge::SemanticAliasNudge, strategy_marker_required::StrategyMarkerRequired,
    trait_first_signatures::TraitFirstSignatures, writing_style::WritingStyle,
};

mockspace_lint_rules::lint_pack! {
    lints: [
        NoAlloc,
        NoStd,
        NoBareOption,
        NoBareResult,
        NoBareNumeric,
        NoBareString,
        NoBareStaticStr,
        NoDynDispatch,
        NoRuntimeSpawn,
        NoRuntimeRegistration,
        NoPublicRawField,
        NoVecInTraitSig,
        StrategyMarkerRequired,
        SemanticAliasNudge,
        TraitFirstSignatures,
        ArvoTypesOnly,
        LintAllowRequiresTaskId,
    ],
    cross_lints: [
        WritingStyle,
    ],
}
